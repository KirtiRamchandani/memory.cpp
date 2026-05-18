use criterion::{criterion_group, criterion_main, Criterion};
use memory_core::{MemoryEngine, NewMemory, RecallQuery};
use tempfile::tempdir;

fn bench_recall(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let engine = MemoryEngine::open_default(dir.path().join("memory.db")).expect("engine");

    for index in 0..1_000 {
        engine
            .remember(
                NewMemory::new(format!(
                    "Memory {index}: local AI tools should optimize for low latency, tiny storage, and clean APIs."
                ))
                .scope("bench")
                .kind("fact"),
            )
            .expect("insert memory");
    }

    c.bench_function("recall_1000_hash_sqlite", |b| {
        b.iter(|| {
            engine
                .recall(
                    RecallQuery::new("low latency local AI memory")
                        .scope("bench")
                        .limit(8),
                )
                .expect("recall")
        });
    });
}

criterion_group!(benches, bench_recall);
criterion_main!(benches);
