"""
Esplora-shaped HTTP shim translating REST calls into electrs Electrum-protocol
requests + bitcoind JSON-RPC. Regtest-only.

Endpoints served under / (callers prepend the full path themselves; the
existing compose healthcheck uses `{base}/blocks/tip/height`):

    GET  blocks/tip/height          -> bitcoind getblockcount
    GET  blocks/tip/hash            -> bitcoind getbestblockhash
    GET  address/<addr>/utxo        -> electrs blockchain.scripthash.listunspent
                                       (addr -> scripthash: SHA-256(scriptPubKey) byte-reversed)
    POST tx                          -> electrs blockchain.transaction.broadcast
                                       (raw tx hex in body; txid out)
    GET  fee-estimates               -> bitcoind estimatesmartfee (sat/vB)

Why a shim at all: electrs serves the Electrum binary protocol over TCP, but
EsploraClient (in bitcoin-wallet-core) speaks plain HTTP REST. Path B (#286)
bundles both processes inside one image so the two-process design stays
self-contained on a single exposed port (50001).

Env vars (defaults in start.sh):
    BITCOIND_RPC_HOST/PORT/USER/PASS
    ELECTRS_HOST/PORT (TCP Electrum)
    SHIM_BIND  (default 0.0.0.0:50001)
"""
from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import os
from typing import Any

import aiohttp
from aiohttp import web

LOG = logging.getLogger("shim")
logging.basicConfig(
    level=os.environ.get("LOG_LEVEL", "INFO"),
    format="%(asctime)s %(levelname)s %(name)s: %(message)s",
)

BITCOIND_RPC_HOST = os.environ.get("BITCOIND_RPC_HOST", "bitcoind")
BITCOIND_RPC_PORT = int(os.environ.get("BITCOIND_RPC_PORT", "18443"))
BITCOIND_RPC_USER = os.environ.get("BITCOIND_RPC_USER", "foo")
BITCOIND_RPC_PASS = os.environ.get("BITCOIND_RPC_PASS", "bar")
BITCOIND_RPC_URL = f"http://{BITCOIND_RPC_HOST}:{BITCOIND_RPC_PORT}/"
ELECTRS_HOST = os.environ.get("ELECTRS_HOST", "127.0.0.1")
# electrs default port inside the container is 50002; shim maps the Esplora
# HTTP shape onto it. 50001 is reserved for shim's external-facing bind.
ELECTRS_PORT = int(os.environ.get("ELECTRS_PORT", "50002"))
SHIM_BIND = os.environ.get("SHIM_BIND", "0.0.0.0:50001")


class ShimError(RuntimeError):
    """Internal error; translated to HTTP 5xx at the route layer."""


# -----------------------------------------------------------------------------
# Electrum protocol client (TCP, line-delimited JSON, request -> response).
# -----------------------------------------------------------------------------
class ElectrumClient:
    """Stateless, one-request-one-response.

    Connection overhead is negligible at regtest scale; opening a fresh socket
    per call avoids needing a pool + graceful failure modes.
    """

    def __init__(
        self,
        host: str = ELECTRS_HOST,
        port: int = ELECTRS_PORT,
        timeout: float = 30.0,
    ) -> None:
        self.host = host
        self.port = port
        self.timeout = timeout

    async def call(self, method: str, *params: Any) -> Any:
        try:
            reader, writer = await asyncio.wait_for(
                asyncio.open_connection(self.host, self.port),
                timeout=self.timeout,
            )
        except (OSError, asyncio.TimeoutError) as e:
            raise ShimError(f"electrs connect {self.host}:{self.port}: {e}") from e

        try:
            req = {"id": 1, "method": method, "params": list(params)}
            writer.write((json.dumps(req) + "\n").encode())
            await writer.drain()
            # Cap line read at 65536 bytes to prevent unbounded allocation
            # if a (hostile or buggy) electrs sends a giant response.
            line = await asyncio.wait_for(reader.readline(65536), timeout=self.timeout)
            resp = json.loads(line)
        except (asyncio.TimeoutError, json.JSONDecodeError) as e:
            raise ShimError(f"electrs {method} i/o: {e}") from e
        finally:
            writer.close()
            try:
                await writer.wait_closed()
            except Exception:  # noqa: BLE001  -- best-effort cleanup
                pass

        if resp.get("error"):
            raise ShimError(f"electrs {method}: {resp['error']}")
        return resp.get("result")


# -----------------------------------------------------------------------------
# Bitcoind JSON-RPC (small wrapper).
# -----------------------------------------------------------------------------
async def bitcoinrpc(method: str, params: list[Any] | None = None) -> Any:
    payload = {"jsonrpc": "1.0", "id": "shim", "method": method, "params": params or []}
    auth = aiohttp.BasicAuth(BITCOIND_RPC_USER, BITCOIND_RPC_PASS)
    async with aiohttp.ClientSession() as sess:
        try:
            async with sess.post(
                BITCOIND_RPC_URL,
                json=payload,
                auth=auth,
                timeout=aiohttp.ClientTimeout(total=10),
            ) as resp:
                body = await resp.text()
        except (aiohttp.ClientError, asyncio.TimeoutError) as e:
            raise ShimError(f"bitcoind {method} transport: {e}") from e
    if resp.status >= 400:
        raise ShimError(f"bitcoind {method} HTTP {resp.status}: {body[:200]}")
    data = json.loads(body)
    if data.get("error"):
        raise ShimError(f"bitcoind {method}: {data['error']}")
    return data["result"]


# -----------------------------------------------------------------------------
# Address decoding (hand-rolled bech32 + base58check). No external deps so the
# runtime image stays slim. Handles P2PKH (m/n/...), P2SH (2...), SegWit bech32
# (tb1/bc1/bcrt1). Taproot (bech32m) is out of scope for regtest use today.
# -----------------------------------------------------------------------------
BECH32_CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"


def _bech32_polymod(values: list[int]) -> int:
    GEN = [0x3B6A57B2, 0x26508E6D, 0x1EA119FA, 0x3D4233AA, 0x2A1462B3]
    chk = 1
    for v in values:
        b = chk >> 25
        chk = ((chk & 0x1FFFFFF) << 5) ^ v
        for i in range(5):
            chk ^= GEN[i] if ((b >> i) & 1) else 0
    return chk


def _bech32_hrp_expand(hrp: str) -> list[int]:
    return [ord(c) >> 5 for c in hrp] + [0] + [ord(c) & 31 for c in hrp]


def _bech32_decode(addr: str) -> tuple[str, int, bytes] | None:
    """Return (hrp, witver, witprog) on success; None on failure."""
    if any(c.isupper() for c in addr) and any(c.islower() for c in addr):
        return None
    addr = addr.lower()
    pos = addr.rfind("1")
    if pos < 1 or pos + 7 > len(addr) or len(addr) > 90:
        return None
    hrp = addr[:pos]
    data = [BECH32_CHARSET.find(c) for c in addr[pos + 1:]]
    if any(d < 0 for d in data):
        return None
    polymod = _bech32_polymod(_bech32_hrp_expand(hrp) + data)
    if polymod != 1:
        return None
    witver = data[0]
    witprog = bytes(data[1:-6])  # strip 6-char checksum suffix
    return hrp, witver, witprog


B58_ALPHABET = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def _base58check_decode(s: str) -> bytes | None:
    # Reject obviously-oversize input early — valid base58 addrs are <= 35 chars.
    if len(s) > 64:
        raise ShimError(f"base58 address too long ({len(s)} chars)")
    num = 0
    for c in s.encode():
        num = num * 58 + B58_ALPHABET.index(c)
    pad = sum(1 for c in s.encode() if c == B58_ALPHABET[0])
    body = num.to_bytes((num.bit_length() + 7) // 8, "big") if num else b""
    decoded = b"\x00" * pad + body
    if len(decoded) < 4:
        return None
    payload, checksum = decoded[:-4], decoded[-4:]
    expected = hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4]
    return payload if checksum == expected else None


def address_to_scriptpubkey(addr: str) -> bytes:
    """Convert a Bitcoin address to its scriptPubKey bytes.

    Supports the address types used on regtest today: P2PKH (m/n/...),
    P2SH (2...), bech32 SegWit v0 (bcrt1.../tb1.../bc1...). Returns bytes.
    """
    # Bech32 mixed-case is invalid per BIP-173; lowercase before prefix check.
    addr_lower = addr.lower()
    # Try bech32 first (cheap signal check on prefix).
    if addr_lower.startswith(("bc1", "tb1", "bcrt1")):
        decoded = _bech32_decode(addr_lower)
        if decoded is None:
            raise ShimError(f"invalid bech32 address {addr!r}")
        _hrp, witver, witprog = decoded
        if len(witprog) not in (20, 32):
            raise ShimError(f"unsupported witness program length {len(witprog)} for {addr!r}")
        # Encode OP_<witver> per Bitcoin Script: OP_0=0x00, OP_1..OP_16=0x51..0x60.
        if witver == 0:
            op_code = 0x00
        elif 1 <= witver <= 16:
            op_code = 0x50 + witver
        else:
            raise ShimError(f"unsupported witness version {witver}")
        return bytes([op_code, len(witprog)]) + witprog

    # Base58 P2PKH or P2SH.
    decoded = _base58check_decode(addr)
    if decoded is None:
        raise ShimError(f"invalid base58check address {addr!r}")
    version = decoded[0]
    h160 = decoded[1:]
    if len(h160) != 20:
        raise ShimError(f"unexpected hash160 length {len(h160)} for {addr!r}")
    # P2PKH version bytes: mainnet 0x00, testnet/regtest 0x6f
    # P2SH version bytes:  mainnet 0x05, testnet/regtest 0xc4
    if version in (0x00, 0x6F):
        return b"\x76\xa9\x14" + h160 + b"\x88\xac"  # OP_DUP OP_HASH160 PUSH20 <h> OP_EQUAL OP_CHECKSIG
    if version in (0x05, 0xC4):
        return b"\xa9\x14" + h160 + b"\x87"  # OP_HASH160 PUSH20 <h> OP_EQUAL
    raise ShimError(f"unsupported address version byte 0x{version:02x} for {addr!r}")


def address_to_scripthash(addr: str) -> str:
    """Electrum scripthash: lower-case hex of sha256(scriptPubKey)[::-1]."""
    script = address_to_scriptpubkey(addr)
    digest = hashlib.sha256(script).digest()
    return digest[::-1].hex()


def err_response(status: int, msg: str) -> web.Response:
    LOG.warning("%d %s", status, msg)
    return web.Response(status=status, text=f"{msg}\n", content_type="text/plain")


# -----------------------------------------------------------------------------
# Route handlers.
# -----------------------------------------------------------------------------
async def handle_blocks_tip_height(_: web.Request) -> web.Response:
    n = await bitcoinrpc("getblockcount")
    return web.Response(text=f"{n}\n")


async def handle_blocks_tip_hash(_: web.Request) -> web.Response:
    h = await bitcoinrpc("getbestblockhash")
    return web.Response(text=f"{h}\n")


async def handle_address_utxos(req: web.Request) -> web.Response:
    addr = req.match_info.get("addr", "")
    if not addr:
        return err_response(400, "missing address")
    try:
        scripthash = address_to_scripthash(addr)
    except ShimError as e:
        return err_response(400, str(e))

    client = ElectrumClient()
    try:
        result = await client.call("blockchain.scripthash.listunspent", scripthash)
    except ShimError as e:
        return err_response(502, str(e))

    # Esplora UTXO shape per blockstream.info REST contract (used by
    # bitcoin-wallet-core/src/chain/esplora.rs:EsploraUtxo).
    out = []
    for u in result:
        height = int(u.get("height", 0))
        out.append({
            "txid": u["tx_hash"],
            "vout": int(u["tx_pos"]),
            "value": int(u["value"]),
            "status": {
                "confirmed": height > 0,
                "block_height": height if height > 0 else 0,
                "block_hash": "",
                "block_time": 0,
            },
        })
    return web.json_response(out)


async def handle_broadcast_tx(req: web.Request) -> web.Response:
    body = await req.read()
    raw_hex = body.decode().strip() if body else ""
    if not raw_hex:
        return err_response(400, "empty body")
    client = ElectrumClient()
    try:
        txid = await client.call("blockchain.transaction.broadcast", raw_hex)
    except ShimError as e:
        return err_response(502, str(e))
    return web.Response(text=f"{txid}\n")


async def handle_fee_estimates(_: web.Request) -> web.Response:
    """Esplora format: {"1": 1.0, ...} in sat/vB. bitcoind returns BTC/kvB.
    1 BTC/kvB == 100,000 sat/vB. Skip entries where bitcoind can't estimate
    rather than returning a malformed response."""
    out: dict[str, float] = {}
    for target in (1, 3, 6, 144):
        try:
            res = await bitcoinrpc("estimatesmartfee", [target])
        except ShimError:
            continue
        rate_btc_kvb = float(res.get("feerate") or 0.0)
        out[str(target)] = rate_btc_kvb * 100_000.0
    return web.json_response(out)


# -----------------------------------------------------------------------------
# App wiring
# -----------------------------------------------------------------------------
def build_app() -> web.Application:
    app = web.Application()
    # Routes mounted under /regtest/api/{...} to match EsploraClient's URL
    # convention (EsploraClient joins relative paths onto base_url ending in
    # "/regtest/api/"; the leading segment is part of the Esplora contract,
    # not a router prefix we can drop).
    app.router.add_get("/regtest/api/blocks/tip/height", handle_blocks_tip_height)
    app.router.add_get("/regtest/api/blocks/tip/hash", handle_blocks_tip_hash)
    app.router.add_get(r"/regtest/api/address/{addr}/utxo", handle_address_utxos)
    app.router.add_post("/regtest/api/tx", handle_broadcast_tx)
    app.router.add_get("/regtest/api/fee-estimates", handle_fee_estimates)
    return app


def main() -> None:
    host, _, port = SHIM_BIND.partition(":")
    app = build_app()
    web.run_app(app, host=host, port=int(port), access_log=None)


if __name__ == "__main__":
    main()
