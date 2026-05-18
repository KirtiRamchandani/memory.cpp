use memory_core::{MemoryEngine, NewMemory, RecallQuery};

fn main() -> memory_core::Result<()> {
    let engine = MemoryEngine::open_default(".memory.cpp/example.db")?;

    engine.remember(
        NewMemory::new("memory.cpp stores durable local memory for AI apps.")
            .scope("example")
            .kind("fact")
            .importance(0.9),
    )?;

    let memories = engine.recall(
        RecallQuery::new("What does memory.cpp store?")
            .scope("example")
            .limit(5),
    )?;

    for memory in memories {
        println!("{:.3} {}", memory.score, memory.memory.summary);
    }

    Ok(())
}
