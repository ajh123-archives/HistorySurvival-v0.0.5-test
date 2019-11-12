use crate::worldgen::WorldGenerationWorker;
use anyhow::Result;
use log::info;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;
use voxel_rs_common::physics::simulation::ServerPhysicsSimulation;
use voxel_rs_common::{
    data::load_data,
    network::{
        messages::{ToClient, ToServer},
        Server, ServerEvent,
    },
    player::RenderDistance,
    world::{
        chunk::{ChunkPos, CompressedChunk},
        BlockPos, World,
    },
    worldgen::DefaultWorldGenerator,
};

mod worldgen;

/// The data that the server stores for every player.
#[derive(Debug, Clone, Default)]
struct PlayerData {
    loaded_chunks: HashSet<ChunkPos>,
    render_distance: RenderDistance,
}

/// Start a new server instance.
pub fn launch_server(mut server: Box<dyn Server>) -> Result<()> {
    info!("Starting server");

    // Load data
    let game_data = load_data("data".into())?;

    let mut world_generator = WorldGenerationWorker::new(
        Box::new(DefaultWorldGenerator::new(&game_data.blocks.clone())),
        game_data.blocks.clone(),
    );

    let mut world = World::new();
    let mut players = HashMap::new();
    let mut physics_simulation = ServerPhysicsSimulation::new();
    // Chunks that are currently generating.
    let mut generating_chunks = HashSet::new();
    let mut update_lightning_chunks = HashSet::new();
    let mut update_lightning_chunks_vec = VecDeque::new();

    info!("Server initialized successfully! Starting server loop");
    loop {
        // Handle messages
        loop {
            match server.receive_event() {
                ServerEvent::NoEvent => break,
                ServerEvent::ClientConnected(id) => {
                    info!("Client connected to the server!");
                    physics_simulation.set_player_input(id, Default::default());
                    players.insert(id, PlayerData::default());
                    server.send(id, ToClient::GameData(game_data.clone()));
                    server.send(id, ToClient::CurrentId(id));
                }
                ServerEvent::ClientDisconnected(id) => {
                    physics_simulation.remove(id);
                    players.remove(&id);
                }
                ServerEvent::ClientMessage(id, message) => match message {
                    ToServer::UpdateInput(input) => {
                        assert!(players.contains_key(&id));
                        physics_simulation.set_player_input(id, input);
                    }
                    ToServer::SetRenderDistance(render_distance) => {
                        assert!(players.contains_key(&id));
                        players.entry(id).and_modify(move |player_data| {
                            player_data.render_distance = render_distance
                        });
                    }
                },
            }
        }

        for chunk in world_generator.get_processed_chunks().into_iter() {
            // Only insert the chunk in the world if it was still being generated.
            if generating_chunks.contains(&chunk.pos) {
                let pos = chunk.pos.clone();
                world.set_chunk(chunk);
                if world.update_highest_opaque_block(pos) {
                    // recompute the light of the 3x3 columns
                    for c_pos in world.chunks.keys() {
                        if c_pos.py <= pos.py && (c_pos.px - pos.px).abs() <= 1 && (c_pos.pz - pos.pz).abs() <= 1 {
                            if !update_lightning_chunks.contains(c_pos){
                                update_lightning_chunks.insert((*c_pos).clone());
                                update_lightning_chunks_vec.push_back((*c_pos).clone());
                            }

                        }
                    }

                } else {
                    // compute only the ligth for the chunk
                    for c_pos in world.chunks.keys() {
                        if (c_pos.py - pos.py).abs() <= 1 && (c_pos.px - pos.px).abs() <= 1 && (c_pos.pz - pos.pz).abs() <= 1 {
                            if !update_lightning_chunks.contains(c_pos){
                                update_lightning_chunks.insert((*c_pos).clone());
                                update_lightning_chunks_vec.push_back((*c_pos).clone());
                            }
                        }
                    }
                }
            }
        }

        // Update light of one chunk at the time
        if !update_lightning_chunks_vec.is_empty(){
            let pos = update_lightning_chunks_vec.pop_front().unwrap();
            let t1 = Instant::now();
            world.update_light(&pos);
            update_lightning_chunks.remove(&pos);
            let t2 = Instant::now();
            println!("Time to compute light : {} ms", (t2-t1).subsec_millis());
        }
        // TODO : Send updated light to the client





        // Tick game
        physics_simulation.step_simulation(Instant::now(), &world);
        // Send updates to players
        for (&player, _) in players.iter() {
            server.send(
                player,
                ToClient::UpdatePhysics((*physics_simulation.get_state()).clone()),
            );
        }

        // Send chunks to players
        let mut player_positions = Vec::new();
        for (player, data) in players.iter_mut() {
            let player_pos = physics_simulation
                .get_state()
                .physics_state
                .players
                .get(player)
                .unwrap()
                .get_camera_position();
            player_positions.push((player_pos, data.render_distance));
            // Send new chunks
            for chunk_pos in data.render_distance.iterate_around_player(player_pos) {
                // The player hasn't received the chunk yet
                if !data.loaded_chunks.contains(&chunk_pos) {
                    if let Some(chunk) = world.get_chunk(chunk_pos) {
                        // Send it to the player if it's in the world
                        server.send(*player, ToClient::Chunk(CompressedChunk::from_chunk(chunk)));
                        data.loaded_chunks.insert(chunk_pos);
                    } else {
                        // Generate the chunk if it's not already generating
                        let actually_inserted = generating_chunks.insert(chunk_pos);
                        if actually_inserted {
                            world_generator.enqueue_chunk(chunk_pos);
                        }
                    }
                }
            }
            // Drop chunks that are too far away
            let render_distance = data.render_distance;
            data.loaded_chunks
                .retain(|chunk_pos| render_distance.is_chunk_visible(player_pos, *chunk_pos));
        }

        // Drop chunks that are far from all players (and update chunk priorities)
        world.chunks.retain(|chunk_pos, _| {
            for (player_position, render_distance) in player_positions.iter() {
                if render_distance.is_chunk_visible(*player_position, *chunk_pos) {
                    return true;
                }
            }
            false
        });
        generating_chunks.retain(|chunk_pos| {
            let mut min_distance = 1_000_000_000;
            let mut retain = false;
            for (player_position, render_distance) in player_positions.iter() {
                if render_distance.is_chunk_visible(*player_position, *chunk_pos) {
                    min_distance = min_distance.min(chunk_pos.squared_euclidian_distance(
                        BlockPos::from(*player_position).containing_chunk_pos(),
                    ));
                    retain = true;
                }
            }
            if !retain {
                world_generator.dequeue_chunk(*chunk_pos);
            } else {
                world_generator.set_chunk_priority(*chunk_pos, min_distance);
            }
            retain
        });

        // Nothing else to do for now :-)
    }
}
