use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
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
}

#[derive(Component)]
struct FireworkParticle {
    velocity: Vec3,
    lifetime: Timer,
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
        .insert_resource(ClearColor(Color::BLACK)) // Night sky
        .init_resource::<FireworkConfig>()
        .add_systems(Startup, setup)
        .add_systems(Update, (
            ui_system, 
            pan_orbit_camera, 
            firework_spawner, 
            rocket_movement, 
            particle_physics,
            draw_gizmos
        ))
        .run();
}

fn setup(
    mut commands: Commands,
) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 15.0, 50.0).looking_at(Vec3::new(0.0, 15.0, 0.0), Vec3::Y),
        OrbitCamera::default(),
    ));

    // Light (optional, particles are emissive but scene might need light)
    commands.spawn((
        PointLight {
            intensity: 100_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 25.0, 0.0),
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
    let x = rng.gen_range(-10.0..10.0);
    let z = rng.gen_range(-10.0..10.0);
    let target_height = rng.gen_range(15.0..30.0);
    
    // Random color
    let hue = rng.gen_range(0.0..360.0);
    let color = Color::hsl(hue, 1.0, 0.5);

    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Sphere::new(0.2)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive: LinearRgba::from(color) * 5.0, // Glow
            ..default()
        })),
        Transform::from_xyz(x, 0.0, z),
        FireworkRocket {
            velocity: Vec3::new(0.0, 15.0, 0.0), // Initial upward velocity
            target_height,
            color,
        },
    ));
}

fn rocket_movement(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    config: Res<FireworkConfig>,
    mut query: Query<(Entity, &mut Transform, &FireworkRocket)>,
) {
    for (entity, mut transform, rocket) in query.iter_mut() {
        // Move up
        transform.translation += rocket.velocity * time.delta_secs();

        // Check if reached target height
        if transform.translation.y >= rocket.target_height {
            // Explode!
            commands.entity(entity).despawn();
            spawn_explosion(&mut commands, &mut meshes, &mut materials, transform.translation, rocket.color, &config);
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
    
    let mesh_handle = meshes.add(Mesh::from(Sphere::new(0.1)));
    let material_handle = materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * 10.0, // Bright glow
        unlit: true, // Don't react to light, just glow
        ..default()
    });

    for _ in 0..config.particle_count {
        // Random direction on sphere
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

        // Physics
        particle.velocity.y -= config.gravity * time.delta_secs(); // Gravity
        particle.velocity *= config.drag; // Drag/Air resistance

        transform.translation += particle.velocity * time.delta_secs();
        
        // Scale down as they die
        let scale = particle.lifetime.fraction_remaining();
        transform.scale = Vec3::splat(scale);
    }
}

fn ui_system(
    mut contexts: EguiContexts, 
    mut config: ResMut<FireworkConfig>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let ctx = contexts.ctx_mut();

    egui::Window::new("Firework Controls").show(ctx, |ui| {
        ui.checkbox(&mut config.auto_launch, "Auto Launch");
        
        ui.add(egui::Slider::new(&mut config.gravity, 0.0..=20.0).text("Gravity"));
        ui.add(egui::Slider::new(&mut config.explosion_force, 1.0..=20.0).text("Explosion Force"));
        ui.add(egui::Slider::new(&mut config.drag, 0.9..=1.0).text("Air Drag"));
        ui.add(egui::Slider::new(&mut config.particle_count, 10..=500).text("Particle Count"));
        
        if ui.button("Launch Rocket").clicked() {
            spawn_rocket(&mut commands, &mut meshes, &mut materials);
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
    // Ground Grid
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
