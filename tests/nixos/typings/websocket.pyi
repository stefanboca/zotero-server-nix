from typing import Any

class WebSocket:
    def recv(self) -> str: ...
    def send(self, payload: str) -> Any: ...

def create_connection(
    url: str,
    timeout: float | None = None,
    sslopt: dict[str, Any] | None = None,
) -> WebSocket: ...
