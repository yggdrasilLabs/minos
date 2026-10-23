// Invariant: only candidate output is re-packed as the judge's input, so judge output has no
// `store_judge_input` (07-CONTEXT D-17).
use minos_core::id::BlobId;
use minos_sandbox::{BlobSink, GuestOutput, Judge, SinkError, Untrusted};

struct Sink;

impl BlobSink for Sink {
    type Error = std::io::Error;

    fn put(&self, bytes: &[u8]) -> Result<BlobId, std::io::Error> {
        Ok(BlobId::of(bytes))
    }
}

fn probe(
    output: Untrusted<GuestOutput<Judge>>,
    sink: &Sink,
) -> Result<BlobId, SinkError<std::io::Error>> {
    output.store_judge_input(sink)
}

fn main() {
    let _probe: fn(Untrusted<GuestOutput<Judge>>, &Sink) -> Result<BlobId, SinkError<std::io::Error>> =
        probe;
    let _ = Sink.put(b"");
}
