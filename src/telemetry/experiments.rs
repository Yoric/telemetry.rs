
/////////////////////////
//
// Experiments

//
// Mutable histograms.
//
// These histograms can only be used by one thread at a time. They are, however, faster
// than immutable histograms.
//
trait MutHistogram<T> {
    //
    // Record a value in this histogram.
    //
    // The value is recorded only if all of the following conditions are met:
    // - `telemetry` is activated; and
    // - this histogram has not expired; and
    // - the histogram is active.
    //
    fn record_mut(&mut self, value: T) {
        self.record_cb_mut(|| Some(value))
    }

    //
    // Record a value in this histogram, as provided by a callback.
    //
    // The callback is triggered only if all of the following conditions are met:
    // - `telemetry` is activated; and
    // - this histogram has not expired; and
    // - the histogram is active.
    //
    // If the callback returns `None`, no value is recorded.
    //
    fn record_cb_mut<F>(&mut self, _: F) where F: FnOnce() -> Option<T>;
}


