use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, DiagnosticsStore};
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use rand::Rng;

#[derive(Resource)]
struct FireworkConfig {
    gravity: f32,
    explosion_force: f32,
    particle_count: usize,
    launch_interval: Timer,
    auto_launch: bool,
    drag: f32,
}

impl Default for FireworkConfig {
    fn default() -> Self {
        Self {
            gravity: 9.8,
            explosion_force: 5.0,
            particle_count: 100,
            launch_interval: Timer::from_seconds(1.0, TimerMode::Repeating),
            auto_launch: true,
            drag: 0.98,
        }
    }
}

#[derive(Component)]
struct FireworkRocket {
    velocity: Vec3,
    target_height: f32,
    color: Color,
    is_cluster: bool,
    trail_timer: Timer, // Control smoke emission rate
}

#[derive(Component)]
struct ClusterBomblet {
    velocity: Vec3,
    fuse_timer: Timer,
    color: Color,
}

#[derive(Component)]
struct FireworkParticle {
    velocity: Vec3,
    lifetime: Timer,
}

#[derive(Component)]
struct TrailParticle {
    lifetime: Timer,
    initial_scale: f32,
}

// Controls the shared material for an explosion to fade it out
#[derive(Component)]
struct ExplosionFade {
    material: Handle<StandardMaterial>,
    lifetime: Timer,
    initial_color: LinearRgba,
    initial_intensity: f32, // For light fading
}

#[derive(Component)]
struct OrbitCamera {
    focus: Vec3,
    radius: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera {
            focus: Vec3::new(0.0, 15.0, 0.0),
            radius: 50.0,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<FireworkConfig>()
        .add_systems(Startup, setup)
        .add_systems(Update, (
            ui_system, 
            pan_orbit_camera, 
            firework_spawner, 
            rocket_movement, 
            cluster_movement,
            particle_physics,
            trail_particle_system, // New system for trails
            explosion_fade_system, 
            draw_gizmos
        ))
        .run();
}

fn setup(
    mut commands: Commands,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 15.0, 50.0).looking_at(Vec3::new(0.0, 15.0, 0.0), Vec3::Y),
        OrbitCamera::default(),
    ));

    // Ambient-ish light to see the ground when no fireworks
    commands.spawn((
        PointLight {
            intensity: 5_000.0,
            shadows_enabled: false,
            range: 100.0,
            ..default()
        },
        Transform::from_xyz(0.0, 20.0, 0.0),
    ));
}

fn firework_spawner(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut config: ResMut<FireworkConfig>,
) {
    config.launch_interval.tick(time.delta());

    if config.auto_launch && config.launch_interval.just_finished() {
        spawn_rocket(&mut commands, &mut meshes, &mut materials);
    }
}

fn spawn_rocket(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();
    let x = rng.gen_range(-15.0..15.0);
    let z = rng.gen_range(-15.0..15.0);
    let target_height = rng.gen_range(20.0..35.0);
    
    let hue = rng.gen_range(0.0..360.0);
    let color = Color::hsl(hue, 1.0, 0.5);
    let is_cluster = rng.gen_bool(0.3);

    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Sphere::new(0.2)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::from(color) * 5.0,
            ..default()
        })),
        Transform::from_xyz(x, 0.0, z),
        FireworkRocket {
            velocity: Vec3::new(0.0, 20.0, 0.0),
            target_height,
            color,
            is_cluster,
            trail_timer: Timer::from_seconds(0.05, TimerMode::Repeating),
        },
    ));
}

fn rocket_movement(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    config: Res<FireworkConfig>,
    mut query: Query<(Entity, &mut Transform, &mut FireworkRocket)>,
) {
    let mut rng = rand::thread_rng();
    
    // Pre-create trail material (white smoke)
    let trail_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.8),
        emissive: LinearRgba::gray(0.5),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let trail_mesh = meshes.add(Mesh::from(Sphere::new(0.1)));

    for (entity, mut transform, mut rocket) in query.iter_mut() {
        transform.translation += rocket.velocity * time.delta_secs();

        // Spawn Trail
        rocket.trail_timer.tick(time.delta());
        if rocket.trail_timer.just_finished() {
             commands.spawn((
                Mesh3d(trail_mesh.clone()),
                MeshMaterial3d(trail_mat.clone()),
                Transform::from_translation(transform.translation + Vec3::new(
                    rng.gen_range(-0.1..0.1), -0.2, rng.gen_range(-0.1..0.1)
                )),
                TrailParticle {
                    lifetime: Timer::from_seconds(0.5, TimerMode::Once),
                    initial_scale: 1.0,
                },
            ));
        }

        if transform.translation.y >= rocket.target_height {
            commands.entity(entity).despawn();
            
            if rocket.is_cluster {
                spawn_cluster(&mut commands, &mut meshes, &mut materials, transform.translation, rocket.color);
            } else {
                spawn_explosion(&mut commands, &mut meshes, &mut materials, transform.translation, rocket.color, &config);
            }
        }
    }
}

fn spawn_cluster(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
    color: Color,
) {
    let mut rng = rand::thread_rng();
    let bomblet_count = rng.gen_range(5..12);
    
    let mesh_handle = meshes.add(Mesh::from(Sphere::new(0.15)));
    let material_handle = materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * 8.0,
        ..default()
    });

    for _ in 0..bomblet_count {
        let vx = rng.gen_range(-5.0..5.0);
        let vy = rng.gen_range(2.0..8.0);
        let vz = rng.gen_range(-5.0..5.0);
        let fuse = rng.gen_range(0.3..0.8);

        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(material_handle.clone()),
            Transform::from_translation(position),
            ClusterBomblet {
                velocity: Vec3::new(vx, vy, vz),
                fuse_timer: Timer::from_seconds(fuse, TimerMode::Once),
                color,
            },
        ));
    }
}

fn cluster_movement(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    config: Res<FireworkConfig>,
    mut query: Query<(Entity, &mut Transform, &mut ClusterBomblet)>,
) {
    for (entity, mut transform, mut bomblet) in query.iter_mut() {
        bomblet.fuse_timer.tick(time.delta());
        
        bomblet.velocity.y -= config.gravity * time.delta_secs();
        transform.translation += bomblet.velocity * time.delta_secs();

        if bomblet.fuse_timer.finished() {
            commands.entity(entity).despawn();
            let mut sub_config = FireworkConfig::default();
            sub_config.particle_count = config.particle_count / 4;
            sub_config.explosion_force = config.explosion_force * 0.7;
            sub_config.gravity = config.gravity;
            sub_config.drag = config.drag;
            
            spawn_explosion(&mut commands, &mut meshes, &mut materials, transform.translation, bomblet.color, &sub_config);
        }
    }
}

fn spawn_explosion(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    position: Vec3,
    color: Color,
    config: &FireworkConfig,
) {
    let mut rng = rand::thread_rng();
    
    // Particle Mesh (Capsule works better for streaks, but Sphere stretched also works)
    let mesh_handle = meshes.add(Mesh::from(Sphere::new(0.05)));
    let material_handle = materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * 10.0,
        unlit: true,
        ..default()
    });

    let light_intensity = 500_000.0;

    // Controller entity with PointLight
    commands.spawn((
        ExplosionFade {
            material: material_handle.clone(),
            lifetime: Timer::from_seconds(2.5, TimerMode::Once),
            initial_color: LinearRgba::from(color),
            initial_intensity: light_intensity,
        },
        PointLight {
            intensity: light_intensity,
            range: 50.0,
            shadows_enabled: false, // Performance
            color: color,
            ..default()
        },
        Transform::from_translation(position),
    ));

    for _ in 0..config.particle_count {
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let phi = rng.gen_range(0.0..std::f32::consts::PI);
        
        let x = phi.sin() * theta.cos();
        let y = phi.sin() * theta.sin();
        let z = phi.cos();
        let dir = Vec3::new(x, y, z);
        
        let speed = rng.gen_range(2.0..config.explosion_force);
        let velocity = dir * speed;
        
        let lifetime = rng.gen_range(1.0..2.5);

        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(material_handle.clone()),
            Transform::from_translation(position),
            FireworkParticle {
                velocity,
                lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
            },
        ));
    }
}

fn explosion_fade_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut ExplosionFade, &mut PointLight)>,
) {
    for (entity, mut fade, mut light) in query.iter_mut() {
        fade.lifetime.tick(time.delta());

        if fade.lifetime.finished() {
            materials.remove(&fade.material);
            commands.entity(entity).despawn();
            continue;
        }

        let progress = fade.lifetime.fraction_remaining(); // 1.0 -> 0.0

        // Fade Material
        if let Some(material) = materials.get_mut(&fade.material) {
            material.emissive = fade.initial_color * (10.0 * progress * progress);
            let mut color = fade.initial_color;
            color.alpha = progress;
            material.base_color = Color::from(color);
        }

        // Fade Light
        light.intensity = fade.initial_intensity * (progress * progress);
    }
}

fn particle_physics(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<FireworkConfig>,
    mut query: Query<(Entity, &mut Transform, &mut FireworkParticle)>,
) {
    for (entity, mut transform, mut particle) in query.iter_mut() {
        particle.lifetime.tick(time.delta());

        if particle.lifetime.finished() {
            commands.entity(entity).despawn();
            continue;
        }

        particle.velocity.y -= config.gravity * time.delta_secs();
        particle.velocity *= config.drag;

        transform.translation += particle.velocity * time.delta_secs();
        
        // STRETCH EFFECT: Look along velocity
        if particle.velocity.length_squared() > 0.1 {
            let look_target = transform.translation + particle.velocity;
            transform.look_at(look_target, Vec3::Y);
            
            // Stretch Z based on speed
            let speed_factor = particle.velocity.length() * 0.05;
            transform.scale = Vec3::new(0.5, 0.5, 1.0 + speed_factor);
        }
    }
}

fn trail_particle_system(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut TrailParticle)>,
) {
    for (entity, mut transform, mut trail) in query.iter_mut() {
        trail.lifetime.tick(time.delta());

        if trail.lifetime.finished() {
            commands.entity(entity).despawn();
            continue;
        }

        let progress = trail.lifetime.fraction_remaining();
        transform.scale = Vec3::splat(trail.initial_scale * progress);
        transform.translation.y += 1.0 * time.delta_secs(); // Drift up slightly
    }
}

fn ui_system(
    mut contexts: EguiContexts, 
    mut config: ResMut<FireworkConfig>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let ctx = contexts.ctx_mut();

    egui::Window::new("Firework Controls").show(ctx, |ui| {
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                ui.heading(format!("FPS: {:.1}", value));
                ui.separator();
            }
        }

        ui.checkbox(&mut config.auto_launch, "Auto Launch");
        
        ui.add(egui::Slider::new(&mut config.gravity, 0.0..=20.0).text("Gravity"));
        ui.add(egui::Slider::new(&mut config.explosion_force, 1.0..=20.0).text("Explosion Force"));
        ui.add(egui::Slider::new(&mut config.drag, 0.9..=1.0).text("Air Drag"));
        ui.add(egui::Slider::new(&mut config.particle_count, 10..=10000).text("Particle Count"));
        
        ui.separator();
        
        if ui.button("Launch Rocket").clicked() {
            spawn_rocket(&mut commands, &mut meshes, &mut materials);
        }

        ui.separator();
        
        if ui.button("Stress Test (Extreme)").clicked() {
            config.particle_count = 2000;
            config.launch_interval = Timer::from_seconds(0.1, TimerMode::Repeating);
            config.auto_launch = true;
            config.explosion_force = 10.0;
        }

        if ui.button("Reset Defaults").clicked() {
            *config = FireworkConfig::default();
        }
    });
}

fn pan_orbit_camera(
    mut ev_motion: EventReader<MouseMotion>,
    mut ev_scroll: EventReader<MouseWheel>,
    input_mouse: Res<ButtonInput<MouseButton>>,
    mut query: Query<(&mut OrbitCamera, &mut Transform)>,
    mut contexts: EguiContexts,
) {
    if contexts.ctx_mut().is_pointer_over_area() {
        return;
    }

    let mut rotation_move = Vec2::ZERO;
    let mut pan = Vec2::ZERO;
    let mut scroll = 0.0;

    if input_mouse.pressed(MouseButton::Right) {
        for ev in ev_motion.read() {
            rotation_move += ev.delta;
        }
    } else if input_mouse.pressed(MouseButton::Middle) {
        for ev in ev_motion.read() {
            pan += ev.delta;
        }
    } else {
        for _ in ev_motion.read() {}
    }

    for ev in ev_scroll.read() {
        scroll += ev.y;
    }

    for (mut orbit, mut transform) in query.iter_mut() {
        if rotation_move.length_squared() > 0.0 {
            let delta_x = rotation_move.x * 0.005;
            let delta_y = rotation_move.y * 0.005;

            let yaw = Quat::from_rotation_y(-delta_x);
            let pitch = Quat::from_rotation_x(-delta_y);

            transform.rotation = yaw * transform.rotation;
            transform.rotation = transform.rotation * pitch;
        } else if pan.length_squared() > 0.0 {
            let right = transform.right();
            let up = transform.up();
            let pan_speed = 0.05;
            let delta = (right * -pan.x + up * pan.y) * pan_speed;
            orbit.focus += delta;
        }

        if scroll.abs() > 0.0 {
            orbit.radius -= scroll * orbit.radius * 0.1;
            orbit.radius = orbit.radius.max(5.0).min(100.0);
        }

        let rot_matrix = Mat3::from_quat(transform.rotation);
        transform.translation = orbit.focus + rot_matrix.mul_vec3(Vec3::new(0.0, 0.0, orbit.radius));
    }
}

fn draw_gizmos(mut gizmos: Gizmos) {
    let grid_size = 20;
    let grid_spacing = 2.0;
    let grid_color = Color::srgb(0.2, 0.2, 0.2); 

    for i in -grid_size..=grid_size {
        let x = i as f32 * grid_spacing;
        gizmos.line(
            Vec3::new(x, 0.0, -grid_size as f32 * grid_spacing), 
            Vec3::new(x, 0.0, grid_size as f32 * grid_spacing), 
            grid_color
        );
        let z = i as f32 * grid_spacing;
        gizmos.line(
            Vec3::new(-grid_size as f32 * grid_spacing, 0.0, z), 
            Vec3::new(grid_size as f32 * grid_spacing, 0.0, z), 
            grid_color
        );
    }
}
