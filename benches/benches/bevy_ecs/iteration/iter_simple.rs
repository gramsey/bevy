use bevy_ecs::prelude::*;
use glam::*;

#[derive(Component, Copy, Clone)]
struct Transform(Mat4);

#[derive(Component, Copy, Clone)]
struct Position(Vec3);

#[derive(Component, Copy, Clone)]
struct Rotation(Vec3);

#[derive(Component, Copy, Clone)]
struct Velocity(Vec3);

pub struct Benchmark<'w>(World, QueryState<(&'w Velocity, &'w mut Position)>);

impl<'w> Benchmark<'w> {
    pub fn new() -> Self {
        let mut world = World::new();

        world.spawn_batch(core::iter::repeat_n(
            (
                Transform(Mat4::from_scale(Vec3::ONE)),
                Position(Vec3::X),
                Rotation(Vec3::X),
                Velocity(Vec3::X),
            ),
            10_000,
        ));

        let query = world.query::<(&Velocity, &mut Position)>();
        Self(world, query)
    }

    #[inline(never)]
    pub fn run(&mut self) {
        for (velocity, mut position) in self.1.iter_mut(&mut self.0) {
            position.0 += velocity.0;
        }
    }
}
