import json
import sys
from pathlib import Path

import websocket


def main() -> None:
    api_key, url = sys.argv[1:]

    connection = websocket.create_connection(url, timeout=30)
    connection.send(
        json.dumps(
            {
                "action": "createSubscriptions",
                "subscriptions": [
                    {
                        "apiKey": api_key,
                        "topics": ["/users/1"],
                    }
                ],
            }
        )
    )

    while True:
        message = json.loads(connection.recv())
        if message.get("event") == "subscriptionsCreated":
            assert message["subscriptions"] == [
                {
                    "apiKey": api_key,
                    "topics": ["/users/1"],
                }
            ], message
            Path("/tmp/stream-ready").touch()
        elif message.get("event") == "topicUpdated":
            assert message["topic"] == "/users/1", message
            Path("/tmp/stream-event").write_text(json.dumps(message))
            return


if __name__ == "__main__":
    main()
