//! A simple example demonstrating how to use Telemetry to measure and
//! store performance data, then eventually dump it to console/disk.

use std::{collections::BTreeMap, convert::TryInto};
use std::fs::File;
use std::io::Write;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

extern crate rustc_serialize;
use rustc_serialize::json::Json;

extern crate telemetry;
use telemetry::plain::*;

extern crate time;

// A stopwatch for microsecond precision.
struct StopwatchUS {
    pub value: Duration,
}
impl StopwatchUS {
    fn span<F>(f: F) -> StopwatchUS
    where
        F: FnOnce(),
    {
        let (duration, _) = time::Duration::time_fn(f);
        StopwatchUS {
            value: duration.try_into().unwrap(),
        }
    }
}
impl telemetry::Flatten for StopwatchUS {
    fn as_u32(&self) -> u32 {
        match self.value.as_micros() {
            x if x >= std::u32::MAX as u128 => std::u32::MAX,
            x => x as u32,
        }
    }
}

struct Histograms {
    /// The duration of execution of a recursive implementation of
    /// Fibonacci's function, in microseconds.
    fibonacci_us: telemetry::plain::Linear<StopwatchUS>,
}

fn fibonacci(i: u32) -> u32 {
    if i == 0 || i == 1 {
        1
    } else {
        fibonacci(i - 1) + fibonacci(i - 2)
    }
}

fn main() {
    let telemetry = telemetry::Service::new(true /* activate immediately */);

    let histograms = Histograms {
        fibonacci_us: telemetry::plain::Linear::new(
            &telemetry,
            "FIBONACCI_DURATION_US".to_string(),
            0,         /* min */
            1_000_000, /* max */
            20,        /* buckets */
        ),
    };

    // Measure a number of durations.
    let mut handles = Vec::new();
    for _ in 1..10 {
        let hist = histograms.fibonacci_us.clone();
        handles.push(thread::spawn(move || {
            hist.record(StopwatchUS::span(|| {
                fibonacci(30);
            }));
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Now look at the histogram.
    let (sender, receiver) = channel();
    telemetry.to_json(
        telemetry::Subset::AllPlain,
        telemetry::SerializationFormat::SimpleJson,
        sender,
    );
    let plain = receiver.recv().unwrap();

    println!("{}", plain);

    // Assemble the histograms payload into a whole.
    // For this example, we are not attempting to match a specific protocol.
    let mut storage = BTreeMap::new();
    storage.insert("plain".to_string(), plain);

    // If we had keyed histograms, we should put them here, too.
    // Now, add metadata.
    storage.insert(
        "application".to_string(),
        Json::String("telemetry example app".to_string()),
    );
    storage.insert(
        "version".to_string(),
        Json::Array(vec![Json::I64(0), Json::I64(1), Json::I64(0)]),
    );

    println!("{}", Json::Object(storage.clone()).pretty());

    // Write to disk.
    let data = format!("{}\n", Json::Object(storage));
    let mut file = File::create("telemetry.json").unwrap();
    file.write_all(&data.into_bytes()).unwrap();
}
