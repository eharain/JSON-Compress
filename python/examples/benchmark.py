"""What the format is worth, on shapes that occur in real systems.

    python examples/benchmark.py

Every figure in the README is produced by running this file, so it can be
checked rather than taken on trust. The interesting column is the last one:
whether the format still helps once gzip has had its turn, which is the only
question that matters for anything served over HTTP.

The generator and the scenarios are the same as in ``javascript/examples/``, so
the first four columns can be compared line for line with the JavaScript and
.NET benchmarks.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path
from typing import Any, List

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from json_compress import byte_length, compress, decompress  # noqa: E402
from json_compress.jsontext import dumps, loads  # noqa: E402
from json_compress.sizes import gzip_size  # noqa: E402

# A deterministic generator, so the numbers are the same on every machine.
_SEED = 20260818
_state = _SEED


def random() -> float:
    global _state
    _state = (_state * 1664525 + 1013904223) & 0xFFFFFFFF
    return _state / 4294967296


def pick(values: List[Any]) -> Any:
    return values[int(random() * len(values))]


def integer(low: int, high: int) -> int:
    return low + int(random() * (high - low + 1))


def rounded(value: float) -> int:
    """Round half away from zero towards positive infinity, as JavaScript does."""
    return math.floor(value + 0.5)


FIRST = [
    "Amara", "Bilal", "Chen", "Dara", "Eesa", "Farah", "Gita", "Hana", "Idris", "Jae",
]
LAST = ["Ahmed", "Bright", "Chowdhury", "Doyle", "Evans", "Fischer", "Gomez", "Haddad"]
CITY = ["London", "Manchester", "Birmingham", "Leeds", "Glasgow", "Cardiff", "Belfast"]


def api_record_set() -> Any:
    return [
        {
            "id": i + 1,
            "firstName": pick(FIRST),
            "lastName": pick(LAST),
            "email": "user" + str(i + 1) + "@example.com",
            "role": pick(["admin", "editor", "viewer", "viewer", "viewer"]),
            "active": random() > 0.2,
            "city": pick(CITY),
            "country": "United Kingdom",
            "createdAt": "2026-0"
            + str(integer(1, 9))
            + "-"
            + str(integer(10, 28))
            + "T09:30:00.000Z",
            "loginCount": integer(0, 4000),
        }
        for i in range(1000)
    ]


def nested_api_response() -> Any:
    return {
        "meta": {
            "page": 1,
            "pageSize": 200,
            "total": 8214,
            "generatedAt": "2026-08-18T12:00:00.000Z",
        },
        "data": [
            {
                "orderId": "ORD-" + str(100000 + i),
                "status": pick(["placed", "picked", "shipped", "delivered"]),
                "currency": "GBP",
                "placedAt": "2026-08-"
                + str(integer(10, 18))
                + "T"
                + str(integer(10, 20))
                + ":15:00.000Z",
                "customer": {
                    "id": integer(1, 5000),
                    "name": pick(FIRST) + " " + pick(LAST),
                    "city": pick(CITY),
                },
                "lines": [
                    {
                        "sku": "SKU-" + str(integer(1000, 9999)),
                        "description": pick(
                            [
                                "Cotton shirt",
                                "Wool scarf",
                                "Leather belt",
                                "Linen trousers",
                            ]
                        ),
                        "quantity": integer(1, 4),
                        "unitPrice": integer(500, 12000) / 100,
                        "taxRate": 0.2,
                    }
                    for _ in range(integer(1, 6))
                ],
            }
            for i in range(200)
        ],
    }


def time_series() -> Any:
    return [
        {
            "t": 1755500000 + i * 60,
            "sensor": pick(["temp-01", "temp-02", "temp-03", "humidity-01"]),
            "unit": "celsius",
            "value": rounded(random() * 3500) / 100,
            "quality": pick(["good", "good", "good", "suspect"]),
        }
        for i in range(2000)
    ]


def structured_logs() -> Any:
    return [
        {
            "level": pick(["info", "info", "info", "warn", "error"]),
            "service": pick(["api-gateway", "orders", "billing", "search"]),
            "env": "production",
            "region": "eu-west-2",
            "message": pick(
                [
                    "request completed",
                    "cache miss",
                    "upstream timeout, retrying",
                    "validation failed",
                ]
            ),
            "durationMs": integer(1, 4000),
            "traceId": "00-"
            + str(integer(100000000, 900000000))
            + "-"
            + str(integer(1000000, 9000000))
            + "-01",
        }
        for _ in range(1500)
    ]


def geojson() -> Any:
    return {
        "type": "FeatureCollection",
        "features": [
            {
                "type": "Feature",
                "geometry": {
                    "type": "Point",
                    "coordinates": [
                        rounded((random() * 8 - 6) * 1e6) / 1e6,
                        rounded((49 + random() * 9) * 1e6) / 1e6,
                    ],
                },
                "properties": {
                    "id": i,
                    "name": pick(CITY) + " site " + str(i),
                    "category": pick(["depot", "store", "hub"]),
                },
            }
            for i in range(400)
        ],
    }


def config_document() -> Any:
    return {
        "version": 3,
        "services": {
            "service-"
            + str(i): {
                "image": "registry.example.com/team/service-" + str(i) + ":1.4." + str(i),
                "replicas": integer(1, 6),
                "resources": {"cpu": "500m", "memory": "512Mi"},
                "env": {
                    "NODE_ENV": "production",
                    "LOG_LEVEL": "info",
                    "REGION": "eu-west-2",
                },
                "ports": [{"container": 8080, "published": 8080 + i, "protocol": "tcp"}],
            }
            for i in range(12)
        },
    }


SCENARIOS: List[tuple] = [
    ("API record set (1,000 users)", api_record_set),
    ("Nested API response (orders with lines)", nested_api_response),
    ("Time series (2,000 readings)", time_series),
    ("Structured logs (1,500 lines)", structured_logs),
    ("GeoJSON feature collection (400 points)", geojson),
    ("Config document (deeply nested, few repeats)", config_document),
]


def kilobytes(count: int) -> str:
    return "%.1f" % (count / 1024)


def main() -> int:
    global _state

    header = (
        "scenario".ljust(46)
        + "minified".rjust(10)
        + "compressed".rjust(12)
        + "saved".rjust(8)
        + "+gzip".rjust(10)
        + "saved".rjust(8)
    )
    print("\n" + header)
    print("-" * len(header))

    for name, build in SCENARIOS:
        _state = _SEED
        value = build()

        minified = dumps(value)
        packed = dumps(compress(value))

        # A benchmark that quietly loses data is not a benchmark.
        if dumps(decompress(loads(packed))) != minified:
            raise SystemExit(name + " did not round-trip")

        minified_bytes = byte_length(minified)
        packed_bytes = byte_length(packed)
        minified_gzip = gzip_size(minified)
        packed_gzip = gzip_size(packed)

        saved = "%.0f%%" % ((1 - packed_bytes / minified_bytes) * 100)
        saved_gzip = "%.0f%%" % ((1 - packed_gzip / minified_gzip) * 100)

        print(
            name.ljust(46)
            + (kilobytes(minified_bytes) + " KB").rjust(10)
            + (kilobytes(packed_bytes) + " KB").rjust(12)
            + saved.rjust(8)
            + (kilobytes(packed_gzip) + " KB").rjust(10)
            + saved_gzip.rjust(8)
        )

    print(
        '\n"saved" is against minified JSON; the gzip columns compare compressed + gzip'
    )
    print("against minified + gzip, which is what an HTTP response actually costs.\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
