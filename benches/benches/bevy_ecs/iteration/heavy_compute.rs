use bevy_ecs::prelude::*;
use bevy_tasks::{ComputeTaskPool, TaskPool};
use criterion::Criterion;
use glam::*;

pub fn heavy_compute(c: &mut Criterion) {
    #[derive(Component, Copy, Clone)]
    struct Position(Vec3);

    #[derive(Component, Copy, Clone)]
    struct Rotation(Vec3);

    #[derive(Component, Copy, Clone)]
    struct Velocity(Vec3);

    #[derive(Component, Copy, Clone)]
    struct Transform(Mat4);

    let mut group = c.benchmark_group("heavy_compute");
    group.warm_up_time(core::time::Duration::from_millis(500));
    group.measurement_time(core::time::Duration::from_secs(4));
    group.bench_function("base", |b| {
        ComputeTaskPool::get_or_init(TaskPool::default);

        let mut world = World::default();

        world.spawn_batch((0..1000).map(|_| {
            (
                Transform(Mat4::from_axis_angle(Vec3::X, 1.2)),
                Position(Vec3::X),
                Rotation(Vec3::X),
                Velocity(Vec3::X),
            )
        }));

        fn sys(mut query: Query<(&mut Position, &mut Transform)>) {
            query.par_iter_mut().for_each(|(mut pos, mut mat)| {
                for _ in 0..100 {
                    mat.0 = mat.0.inverse();
                }

                pos.0 = mat.0.transform_vector3(pos.0);
            });
        }

        let mut system = IntoSystem::into_system(sys);
        system.initialize(&mut world);

        b.iter(move || system.run((), &mut world));
    });
    group.finish();
}
