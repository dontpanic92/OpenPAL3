"""HTTP client for the PAL4 agent server.

Wraps every endpoint the planner uses with typed helpers, plus a
small amount of glue (`fire_trigger_sync`) for the common
"fire-and-wait-and-trace" sequence the planner does on each step.

Only depends on the stdlib so the planner can be run from a fresh
Python install with no `pip install` step.
"""

from __future__ import annotations

import dataclasses
import json
import time
import urllib.error
import urllib.request
from typing import Any, Iterable, Optional


class AgentError(Exception):
    """Raised when the agent server returns a structured error
    response. `kind` matches the server's `AgentErrorKind` snake-case
    name (e.g. `"conflict"`, `"bad_request"`).
    """

    def __init__(self, status: int, kind: str, message: str):
        super().__init__(f"[{status} {kind}] {message}")
        self.status = status
        self.kind = kind
        self.message = message


@dataclasses.dataclass
class FireResult:
    """Subset of `FireTriggerResponse` the planner needs."""

    name: str
    settled: bool
    trace_seq_start: Optional[int]
    trace_seq_end: Optional[int]
    waited_frames: int
    current_script_fn: Optional[str]


class Client:
    """Tiny synchronous HTTP client for the agent server.

    Built on `urllib.request` to keep the dependency footprint at
    zero. Retries 409 (conflict) responses with exponential backoff —
    these come from "a script is already running" races between
    rapid-fire planner steps.
    """

    def __init__(
        self,
        base_url: str = "http://127.0.0.1:8765",
        timeout: float = 10.0,
        token: Optional[str] = None,
        retry_409: int = 6,
    ):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout
        self.token = token
        self.retry_409 = max(0, retry_409)

    # ---- low-level transport --------------------------------------------

    def _request(self, method: str, path: str, body: Any = None) -> Any:
        url = f"{self.base_url}{path}"
        data = None
        headers = {}
        if body is not None:
            data = json.dumps(body).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        req = urllib.request.Request(url, method=method, data=data, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            raw = e.read().decode("utf-8", "replace")
            try:
                env = json.loads(raw)
                if env.get("type") == "error":
                    err = env.get("data") or {}
                    raise AgentError(
                        e.code,
                        err.get("kind", "internal"),
                        err.get("message", raw),
                    ) from None
            except json.JSONDecodeError:
                pass
            raise AgentError(e.code, "internal", raw) from None

    def _retry_409(self, method: str, path: str, body: Any = None) -> Any:
        delay = 0.1
        for attempt in range(self.retry_409 + 1):
            try:
                return self._request(method, path, body)
            except AgentError as e:
                if e.kind != "conflict" or attempt == self.retry_409:
                    raise
                time.sleep(delay)
                delay = min(delay * 2.0, 1.0)

    # ---- observability ---------------------------------------------------

    def state(self) -> dict:
        env = self._request("GET", "/v1/state")
        return env.get("data", {})

    def globals(self, start: int = 0, limit: Optional[int] = None) -> list[int]:
        path = f"/v1/script/globals?start={start}"
        if limit is not None:
            path += f"&limit={limit}"
        env = self._request("GET", path)
        return list(env.get("data", {}).get("globals", []))

    def scene_triggers(self) -> list[dict]:
        env = self._request("GET", "/v1/scene/triggers")
        return list(env.get("data", {}).get("triggers", []))

    def scene_objects(self) -> dict:
        env = self._request("GET", "/v1/scene/objects")
        return env.get("data", {})

    def log_tail(self, after_seq: int = 0, n: int = 256) -> dict:
        env = self._request("GET", f"/v1/log/tail?after_seq={after_seq}&n={n}")
        return env.get("data", {})

    # ---- trace -----------------------------------------------------------

    def trace_start(self, reset: bool = True, capacity: Optional[int] = None) -> None:
        body: dict = {"reset": reset}
        if capacity is not None:
            body["capacity"] = capacity
        self._request("POST", "/v1/script/trace/start", body)

    def trace_stop(self) -> None:
        self._request("POST", "/v1/script/trace/stop", {})

    def trace_drain(self, after_seq: int = 0, n: int = 1024) -> dict:
        env = self._request("GET", f"/v1/script/trace/drain?after_seq={after_seq}&n={n}")
        return env.get("data", {})

    def trace_drain_all(self, after_seq: int = 0, batch: int = 1024) -> Iterable[dict]:
        """Yield every trace event with `seq > after_seq`, paging
        through the ring until the cursor stops advancing.
        """
        cursor = after_seq
        while True:
            page = self.trace_drain(cursor, batch)
            events = page.get("events", [])
            if not events:
                return
            for ev in events:
                yield ev
            new_cursor = page.get("next_seq", cursor)
            if new_cursor <= cursor:
                return
            cursor = new_cursor

    # ---- control ---------------------------------------------------------

    def fast_forward(self, on: bool) -> None:
        self._request("POST", "/v1/time/fast_forward", {"on": on})

    def dialog_advance(self) -> None:
        self._request("POST", "/v1/dialog/advance", {})

    def dialog_choose(self, index: int) -> None:
        self._request("POST", "/v1/dialog/choose", {"index": index})

    def fire_trigger(
        self,
        name: str,
        wait_until_idle: bool = False,
        collect_trace: bool = False,
        timeout_ms: Optional[int] = None,
    ) -> Optional[FireResult]:
        body: dict = {
            "name": name,
            "wait_until_idle": wait_until_idle,
            "collect_trace": collect_trace,
        }
        if timeout_ms is not None:
            body["timeout_ms"] = timeout_ms
        env = self._retry_409("POST", "/v1/scene/fire_trigger", body)
        if env.get("type") == "fire_trigger":
            d = env.get("data", {})
            return FireResult(
                name=d.get("name", name),
                settled=bool(d.get("settled", False)),
                trace_seq_start=d.get("trace_seq_start"),
                trace_seq_end=d.get("trace_seq_end"),
                waited_frames=int(d.get("waited_frames", 0)),
                current_script_fn=d.get("current_script_fn"),
            )
        return None

    def fire_trigger_sync(self, name: str, timeout_ms: int = 5000) -> FireResult:
        """Convenience: fire-and-wait-and-trace.

        Equivalent to `fire_trigger(name, wait_until_idle=True,
        collect_trace=True, timeout_ms=...)` but raises when the
        engine returns a non-fire-trigger response (e.g. an immediate
        error). The planner uses this for every step.
        """
        result = self.fire_trigger(
            name,
            wait_until_idle=True,
            collect_trace=True,
            timeout_ms=timeout_ms,
        )
        if result is None:
            raise AgentError(
                500,
                "internal",
                f"fire_trigger {name!r}: expected fire_trigger response",
            )
        return result

    def interact(self, name: str) -> None:
        self._retry_409("POST", "/v1/object/interact", {"name": name})

    def teleport(self, player: int, pos: tuple[float, float, float]) -> None:
        self._request(
            "POST",
            "/v1/player/teleport",
            {"player": player, "pos": list(pos)},
        )

    def save(self, slot: int) -> None:
        self._request("POST", "/v1/save", {"slot": slot})

    def load(self, slot: int) -> None:
        self._request("POST", "/v1/load", {"slot": slot})
