use std::{fs, path::PathBuf, sync::Arc};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use parking_lot::RwLock;
use pumpkin_data::BlockDirection;
use pumpkin_util::math::{position::BlockPos, vector2::Vector2};
use pumpkin_world::{
    chunk::ChunkData,
    dimension::Dimension,
    global_path,
    level::Level,
    world::{BlockAccessor, BlockRegistryExt},
};

fn test_reads(level: &Arc<Level>, positions: Vec<Vector2<i32>>) {
    let level = level.clone();
    let receiver = level.receive_chunks(positions);

    loop {
        match receiver.recv() {
            Ok(_) => {}
            Err(err) => println!("error receiving chunk {err:?}"),
        }
    }
    // while let Ok(x) = receiver.recv() {
    //     println!("recieved")
    //     // Don't compile me away!
    //     let _ = x;
    // }
}

/*
fn test_reads_parallel(level: &Arc<Level>, positions: Vec<Vector2<i32>>, threads: usize) {
    let mut tasks = JoinSet::new();

    // we write non overlapping chunks to avoid conflicts or level cache
    // also we use `.rev()` to get the external radius first, avoiding
    // multiple files on the same request.
    for positions in positions.chunks(CHUNKS_ON_PARALLEL).rev().take(threads) {
        let level = level.clone();
        let positions = positions.to_vec();
        tasks.spawn(async move {
            test_reads(&level, positions.clone());
        });
    }

    let _ = tasks.join_all();
}
*/

fn test_writes(level: &Arc<Level>, chunks: Vec<(Vector2<i32>, Arc<RwLock<ChunkData>>)>) {
    level.write_chunks(chunks);
}

/*
fn test_writes_parallel(
    level: &Arc<Level>,
    chunks: Vec<(Vector2<i32>, Arc<RwLock<ChunkData>>)>,
    threads: usize,
) {
    let mut tasks = JoinSet::new();

    // we write non overlapping chunks to avoid conflicts or level cache
    // also we use `.rev()` to get the external radius first, avoiding
    // multiple files on the same request.
    for chunks in chunks.chunks(CHUNKS_ON_PARALLEL).rev().take(threads) {
        let level = level.clone();
        let chunks = chunks.to_vec();
        tasks.spawn(async move {
            test_writes(&level, chunks);
        });
    }

    let _ = tasks.join_all();
}
*/

// -16..16 == 32 chunks, 32*32 == 1024 chunks
const MIN_CHUNK: i32 = -16;
const MAX_CHUNK: i32 = 16;

// How many chunks to use on parallel tests
//const CHUNKS_ON_PARALLEL: usize = 32;

struct BlockRegistry;

impl BlockRegistryExt for BlockRegistry {
    fn can_place_at(
        &self,
        _block: &pumpkin_data::Block,
        _block_accessor: &dyn BlockAccessor,
        _block_pos: &BlockPos,
        _face: BlockDirection,
    ) -> bool {
        true
    }
}

fn initialize_level(root_dir: PathBuf) -> Vec<(Vector2<i32>, Arc<RwLock<ChunkData>>)> {
    println!("Initializing data...");
    // Initial writes
    let mut chunks = Vec::new();
    let block_registry = Arc::new(BlockRegistry);

    // Our data dir is empty, so we're generating new chunks here
    let level_to_save =
        Level::from_root_folder(root_dir.clone(), block_registry, 123, Dimension::Overworld);
    println!("Level Seed is: {}", level_to_save.seed.0);

    let level_to_fetch = level_to_save.clone();
    let chunks_to_generate = (MIN_CHUNK..MAX_CHUNK)
        .flat_map(|x| (MIN_CHUNK..MAX_CHUNK).map(move |z| Vector2::new(x, z)))
        .collect::<Vec<_>>();
    let receiver = level_to_fetch.receive_chunks(chunks_to_generate);

    while let Ok((chunk, _)) = receiver.recv() {
        let pos = chunk.read().position;
        chunks.push((pos, chunk));
    }
    level_to_save.write_chunks(chunks.clone());

    // Sort by distance from origin to ensure a fair selection
    // when using a subset of the total chunks for the benchmarks
    chunks.sort_unstable_by_key(|chunk| (chunk.0.x * chunk.0.x) + (chunk.0.y * chunk.0.y));
    chunks
}

// Depends on config options from `./config`
/*
// This doesn't really test anything...
fn bench_chunk_io_parallel(c: &mut Criterion) {
    // System temp dirs are in-memory, so we can't use temp_dir
    let root_dir = global_path!("./bench_root_tmp");
    let _ = fs::remove_dir_all(&root_dir); // delete if it exists
    fs::create_dir(&root_dir).unwrap(); // create the directory

    let async_handler = tokio::runtime::Builder::new_multi_thread().build().unwrap();

    let chunks = initialize_level(&async_handler, root_dir.clone());
    let positions = chunks.iter().map(|(pos, _)| *pos).collect::<Vec<_>>();

    let iters = [1, 2, 8, 32];

    let mut write_group_parallel = c.benchmark_group("write_chunks");
    for n_requests in iters {
        let root_dir = root_dir.clone();

        write_group_parallel.bench_with_input(
            BenchmarkId::new("Parallel", n_requests),
            &chunks,
            |b, parallel_chunks| {
                let chunks = parallel_chunks.to_vec();
                b.to_async(&async_handler).iter(async || {
                    let level = Arc::new(Level::from_root_folder(root_dir.clone()));
                    test_writes_parallel(&level, chunks.clone(), n_requests)
                })
            },
        );
    }
    write_group_parallel.finish();

    let mut read_group = c.benchmark_group("read_chunks");
    for n_requests in iters {
        let root_dir = root_dir.clone();


        read_group.bench_with_input(
            BenchmarkId::new("Parallel", n_requests),
            &positions,
            |b, positions| {
                let positions = positions.to_vec();
                b.to_async(&async_handler).iter(async || {
                    let level = Arc::new(Level::from_root_folder(root_dir.clone()));
                    test_reads_parallel(&level, positions.clone(), n_requests)
                })
            },
        );
    }
    read_group.finish();

    fs::remove_dir_all(&root_dir).unwrap(); // cleanup

}
*/

// Depends on config options from `./config`
fn bench_chunk_io(c: &mut Criterion) {
    // System temp dirs are in-memory, so we can't use temp_dir
    let root_dir = global_path!("./bench_root_tmp");
    let _ = fs::remove_dir_all(&root_dir); // delete it if it exists
    fs::create_dir(&root_dir).unwrap(); // create the directory

    let chunks = initialize_level(root_dir.clone());
    let positions = chunks.iter().map(|(pos, _)| *pos).collect::<Vec<_>>();

    let iters = [16, 64, 256, 512];
    // These test worst case: no caching done by `Level`
    // testing with 16, 64, 256 chunks
    let mut write_group = c.benchmark_group("write_chunks");
    for n_chunks in iters {
        let chunks = &chunks[..n_chunks];
        let root_dir = root_dir.clone();
        assert!(
            chunks.len() == n_chunks,
            "Expected {} chunks, got {}",
            n_chunks,
            chunks.len()
        );
        let block_registry = Arc::new(BlockRegistry);

        write_group.bench_with_input(
            BenchmarkId::new("Single", n_chunks),
            &chunks,
            |b, chunks| {
                b.iter(|| {
                    let level = Arc::new(Level::from_root_folder(
                        root_dir.clone(),
                        block_registry.clone(),
                        123,
                        Dimension::Overworld,
                    ));
                    test_writes(&level, chunks.to_vec())
                })
            },
        );
    }
    write_group.finish();

    // These test worst case: no caching done by `Level`
    // testing with 16, 64, 256 chunks
    let mut read_group = c.benchmark_group("read_chunks");
    for n_chunks in iters {
        let positions = &positions[..n_chunks];
        let root_dir = root_dir.clone();
        assert!(
            positions.len() == n_chunks,
            "Expected {} chunks, got {}",
            n_chunks,
            positions.len()
        );
        let block_registry = Arc::new(BlockRegistry);

        read_group.bench_with_input(
            BenchmarkId::new("Single", n_chunks),
            &positions,
            |b, positions| {
                b.iter(|| {
                    let level = Arc::new(Level::from_root_folder(
                        root_dir.clone(),
                        block_registry.clone(),
                        123,
                        Dimension::Overworld,
                    ));
                    test_reads(&level, positions.to_vec())
                })
            },
        );
    }
    read_group.finish();

    fs::remove_dir_all(&root_dir).unwrap(); // cleanup
}

criterion_group!(benches, bench_chunk_io);
criterion_main!(benches);
