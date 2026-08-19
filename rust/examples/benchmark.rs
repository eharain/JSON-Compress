//! Reports what the format is worth, on shapes that occur in real systems.
//!
//! ```text
//! cargo run --release --example benchmark --features gzip
//! ```
//!
//! Every figure in the README is produced by running this file, so it can be
//! checked rather than taken on trust. The interesting column is the last one:
//! whether the format still helps once gzip has had its turn, which is the only
//! question that matters for anything served over HTTP.
//!
//! The generator and the scenarios are the same as in `javascript/examples/`, so
//! the first four columns can be compared line for line with the JavaScript,
//! .NET, Python, PHP and Go benchmarks.

use json_compress::{format_json, gzip_size, to_string, Object, Value};

/// A deterministic generator, so the numbers are the same on every machine.
const SEED: u32 = 20260818;

struct Random {
    state: u32,
}

impl Random {
    fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        self.state as f64 / 4294967296.0
    }

    fn pick(&mut self, values: &[&str]) -> String {
        values[(self.next() * values.len() as f64) as usize].to_owned()
    }

    fn integer(&mut self, low: i64, high: i64) -> f64 {
        (low + (self.next() * (high - low + 1) as f64) as i64) as f64
    }
}

/// Rounds half away from zero towards positive infinity, as JavaScript's
/// `Math.round` does.
fn rounded(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// Writes a whole float the way the other benchmarks write it, so the generated
/// documents are identical across implementations.
fn number(value: f64) -> String {
    json_compress::format_float(value)
}

macro_rules! obj {
    ($($key:expr => $value:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut out = Object::new();
        $( out.insert($key, $value); )*
        Value::Object(out)
    }};
}

const FIRST: [&str; 10] = [
    "Amara", "Bilal", "Chen", "Dara", "Eesa", "Farah", "Gita", "Hana", "Idris", "Jae",
];
const LAST: [&str; 8] = [
    "Ahmed",
    "Bright",
    "Chowdhury",
    "Doyle",
    "Evans",
    "Fischer",
    "Gomez",
    "Haddad",
];
const CITIES: [&str; 7] = [
    "London",
    "Manchester",
    "Birmingham",
    "Leeds",
    "Glasgow",
    "Cardiff",
    "Belfast",
];

fn api_record_set(random: &mut Random) -> Value {
    Value::Array(
        (0..1000)
            .map(|at| {
                obj! {
                    "id" => (at + 1) as f64,
                    "firstName" => random.pick(&FIRST),
                    "lastName" => random.pick(&LAST),
                    "email" => format!("user{}@example.com", at + 1),
                    "role" => random.pick(&["admin", "editor", "viewer", "viewer", "viewer"]),
                    "active" => random.next() > 0.2,
                    "city" => random.pick(&CITIES),
                    "country" => "United Kingdom",
                    "createdAt" => format!(
                        "2026-0{}-{}T09:30:00.000Z",
                        number(random.integer(1, 9)),
                        number(random.integer(10, 28))
                    ),
                    "loginCount" => random.integer(0, 4000),
                }
            })
            .collect::<Vec<_>>(),
    )
}

fn nested_api_response(random: &mut Random) -> Value {
    let data: Vec<Value> = (0..200)
        .map(|at| {
            // The generator is shared with the other implementations, so the
            // values have to be drawn in the order the JavaScript object literal
            // draws them - the rows would otherwise differ and the figures with
            // them.
            let status = random.pick(&["placed", "picked", "shipped", "delivered"]);
            let placed_at = format!(
                "2026-08-{}T{}:15:00.000Z",
                number(random.integer(10, 18)),
                number(random.integer(10, 20))
            );
            let customer = obj! {
                "id" => random.integer(1, 5000),
                "name" => format!("{} {}", random.pick(&FIRST), random.pick(&LAST)),
                "city" => random.pick(&CITIES),
            };

            let line_count = random.integer(1, 6) as usize;
            let lines: Vec<Value> = (0..line_count)
                .map(|_| {
                    obj! {
                        "sku" => format!("SKU-{}", number(random.integer(1000, 9999))),
                        "description" => random.pick(&[
                            "Cotton shirt",
                            "Wool scarf",
                            "Leather belt",
                            "Linen trousers",
                        ]),
                        "quantity" => random.integer(1, 4),
                        "unitPrice" => random.integer(500, 12000) / 100.0,
                        "taxRate" => 0.2,
                    }
                })
                .collect();

            obj! {
                "orderId" => format!("ORD-{}", 100000 + at),
                "status" => status,
                "currency" => "GBP",
                "placedAt" => placed_at,
                "customer" => customer,
                "lines" => Value::Array(lines),
            }
        })
        .collect();

    obj! {
        "meta" => obj! {
            "page" => 1.0,
            "pageSize" => 200.0,
            "total" => 8214.0,
            "generatedAt" => "2026-08-18T12:00:00.000Z",
        },
        "data" => Value::Array(data),
    }
}

fn time_series(random: &mut Random) -> Value {
    Value::Array(
        (0..2000)
            .map(|at| {
                obj! {
                    "t" => (1755500000 + at * 60) as f64,
                    "sensor" => random.pick(&["temp-01", "temp-02", "temp-03", "humidity-01"]),
                    "unit" => "celsius",
                    "value" => rounded(random.next() * 3500.0) / 100.0,
                    "quality" => random.pick(&["good", "good", "good", "suspect"]),
                }
            })
            .collect::<Vec<_>>(),
    )
}

fn structured_logs(random: &mut Random) -> Value {
    Value::Array(
        (0..1500)
            .map(|_| {
                obj! {
                    "level" => random.pick(&["info", "info", "info", "warn", "error"]),
                    "service" => random.pick(&["api-gateway", "orders", "billing", "search"]),
                    "env" => "production",
                    "region" => "eu-west-2",
                    "message" => random.pick(&[
                        "request completed",
                        "cache miss",
                        "upstream timeout, retrying",
                        "validation failed",
                    ]),
                    "durationMs" => random.integer(1, 4000),
                    "traceId" => format!(
                        "00-{}-{}-01",
                        number(random.integer(100000000, 900000000)),
                        number(random.integer(1000000, 9000000))
                    ),
                }
            })
            .collect::<Vec<_>>(),
    )
}

fn geo_json(random: &mut Random) -> Value {
    let features: Vec<Value> = (0..400)
        .map(|at| {
            obj! {
                "type" => "Feature",
                "geometry" => obj! {
                    "type" => "Point",
                    "coordinates" => Value::Array(vec![
                        Value::Float(rounded((random.next() * 8.0 - 6.0) * 1e6) / 1e6),
                        Value::Float(rounded((49.0 + random.next() * 9.0) * 1e6) / 1e6),
                    ]),
                },
                "properties" => obj! {
                    "id" => at as f64,
                    "name" => format!("{} site {at}", random.pick(&CITIES)),
                    "category" => random.pick(&["depot", "store", "hub"]),
                },
            }
        })
        .collect();

    obj! {
        "type" => "FeatureCollection",
        "features" => Value::Array(features),
    }
}

fn config_document(random: &mut Random) -> Value {
    let mut services = Object::new();
    for at in 0..12 {
        services.insert(
            format!("service-{at}"),
            obj! {
                "image" => format!("registry.example.com/team/service-{at}:1.4.{at}"),
                "replicas" => random.integer(1, 6),
                "resources" => obj! { "cpu" => "500m", "memory" => "512Mi" },
                "env" => obj! {
                    "NODE_ENV" => "production",
                    "LOG_LEVEL" => "info",
                    "REGION" => "eu-west-2",
                },
                "ports" => Value::Array(vec![obj! {
                    "container" => 8080.0,
                    "published" => (8080 + at) as f64,
                    "protocol" => "tcp",
                }]),
            },
        );
    }
    obj! { "version" => 3.0, "services" => Value::Object(services) }
}

type Scenario = (&'static str, fn(&mut Random) -> Value);

const SCENARIOS: [Scenario; 6] = [
    ("API record set (1,000 users)", api_record_set),
    (
        "Nested API response (orders with lines)",
        nested_api_response,
    ),
    ("Time series (2,000 readings)", time_series),
    ("Structured logs (1,500 lines)", structured_logs),
    ("GeoJSON feature collection (400 points)", geo_json),
    (
        "Config document (deeply nested, few repeats)",
        config_document,
    ),
];

fn main() {
    let header = format!(
        "{:<46}{:>10}{:>12}{:>8}{:>10}{:>8}",
        "scenario", "minified", "compressed", "saved", "+gzip", "saved"
    );
    println!("\n{header}");
    println!("{}", "-".repeat(header.len()));

    for (name, build) in SCENARIOS {
        let mut random = Random { state: SEED };
        let value = build(&mut random);

        let minified = format_json(&value);
        let packed = to_string(&value).expect("compressing");

        // A benchmark that quietly loses data is not a benchmark.
        let restored = json_compress::from_str(&packed).expect("restoring");
        assert_eq!(
            format_json(&restored),
            minified,
            "{name} did not round-trip"
        );

        let minified_gzip = gzip_size(minified.as_bytes());
        let packed_gzip = gzip_size(packed.as_bytes());

        println!(
            "{:<46}{:>10}{:>12}{:>8}{:>10}{:>8}",
            name,
            kilobytes(minified.len()),
            kilobytes(packed.len()),
            saved(packed.len(), minified.len()),
            kilobytes(packed_gzip),
            saved(packed_gzip, minified_gzip),
        );
    }

    println!("\n\"saved\" is against minified JSON; the gzip columns compare compressed + gzip");
    println!("against minified + gzip, which is what an HTTP response actually costs.");
}

fn kilobytes(count: usize) -> String {
    format!("{:.1} KB", count as f64 / 1024.0)
}

fn saved(packed: usize, plain: usize) -> String {
    if plain == 0 {
        return "-".to_owned();
    }
    format!("{:.0}%", (1.0 - packed as f64 / plain as f64) * 100.0)
}
