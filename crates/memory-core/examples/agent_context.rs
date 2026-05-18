use memory_core::{MemoryEngine, NewMemory, RecallQuery};

fn main() -> memory_core::Result<()> {
    let engine = MemoryEngine::open_default(".memory.cpp/agent.db")?;

    engine.remember(
        NewMemory::new(
            "The user likes terse APIs, local-first tools, and benchmark-driven design.",
        )
        .scope("user")
        .kind("preference")
        .importance(0.95),
    )?;

    let context = engine.context(
        RecallQuery::new("How should I design the next API?")
            .scope("user")
            .limit(4),
        600,
    )?;

    println!("{}", context.text);
    Ok(())
}
