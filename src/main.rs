use bevy::prelude::*;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::render::render_resource::PrimitiveTopology;
use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::Indices;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use truck_modeling::{builder, Point3, Vector3, Rad, EuclideanSpace};
use truck_meshalgo::tessellation::{MeshableShape, MeshedShape};
use truck_polymesh::PolygonMesh;

#[derive(Resource)]
struct CadParams {
    radius: f64,
    height: f64,
    resolution: f64,
    regenerate: bool,
}

impl Default for CadParams {
    fn default() -> Self {
        Self {
            radius: 1.0,
            height: 2.0,
            resolution: 0.05,
            regenerate: true,
        }
    }
}

#[derive(Component)]
struct CadModel;

#[derive(Component)]
struct OrbitCamera {
    focus: Vec3,
    radius: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera {
            focus: Vec3::ZERO,
            radius: 10.0,
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .insert_resource(ClearColor(Color::srgb(0.9, 0.9, 0.9))) // Light gray background
        .init_resource::<CadParams>()
        .add_systems(Startup, setup)
        .add_systems(Update, (ui_system, update_mesh_system, pan_orbit_camera))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    let translation = Vec3::new(-5.0, 5.0, 5.0);
    let radius = translation.length();
    
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(translation).looking_at(Vec3::ZERO, Vec3::Y),
        OrbitCamera {
            radius,
            ..default()
        },
    ));

    // Light
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // Placeholder mesh entity
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Cuboid::default()))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.5, 0.3),
            perceptual_roughness: 0.8,
            ..default()
        })),
        CadModel,
    ));
}

fn pan_orbit_camera(
    mut ev_motion: EventReader<MouseMotion>,
    mut ev_scroll: EventReader<MouseWheel>,
    input_mouse: Res<ButtonInput<MouseButton>>,
    mut query: Query<(&mut OrbitCamera, &mut Transform)>,
    mut contexts: EguiContexts,
) {
    // block camera control if over UI
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
        // Consume events to prevent buildup
        for _ in ev_motion.read() {}
    }

    for ev in ev_scroll.read() {
        scroll += ev.y;
    }

    for (mut orbit, mut transform) in query.iter_mut() {
        if rotation_move.length_squared() > 0.0 {
            let delta_x = rotation_move.x * 0.005; // sensitivity
            let delta_y = rotation_move.y * 0.005;

            let yaw = Quat::from_rotation_y(-delta_x);
            let pitch = Quat::from_rotation_x(-delta_y);

            transform.rotation = yaw * transform.rotation; // rotate around global Y
            transform.rotation = transform.rotation * pitch; // rotate around local X
        } else if pan.length_squared() > 0.0 {
            let right = transform.right();
            let up = transform.up();
            let pan_speed = 0.01;
            let delta = (right * -pan.x + up * pan.y) * pan_speed;
            orbit.focus += delta;
        }

        if scroll.abs() > 0.0 {
            orbit.radius -= scroll * orbit.radius * 0.1; // exponential zoom
            orbit.radius = orbit.radius.max(0.1).min(50.0);
        }

        // Update translation based on rotation, radius, and focus
        let rot_matrix = Mat3::from_quat(transform.rotation);
        transform.translation = orbit.focus + rot_matrix.mul_vec3(Vec3::new(0.0, 0.0, orbit.radius));
    }
}

fn ui_system(
    mut contexts: EguiContexts, 
    mut params: ResMut<CadParams>,
    mut camera_query: Query<(&mut OrbitCamera, &mut Transform)>,
) {
    let ctx = contexts.ctx_mut();
    egui::Window::new("CAD Controls").show(ctx, |ui| {
        if ui.add(egui::Slider::new(&mut params.radius, 0.1..=5.0).text("Radius")).changed() {
            params.regenerate = true;
        }
        if ui.add(egui::Slider::new(&mut params.height, 0.1..=5.0).text("Height")).changed() {
            params.regenerate = true;
        }
        if ui.add(egui::Slider::new(&mut params.resolution, 0.01..=0.5).text("Tessellation")).changed() {
            params.regenerate = true;
        }
        
        ui.separator();
        
        ui.horizontal(|ui| {
            if ui.button("Regenerate").clicked() {
                params.regenerate = true;
            }
            if ui.button("Reset Model").clicked() {
                *params = CadParams::default();
            }
        });

        ui.separator();

        if ui.button("Reset Camera").clicked() {
            for (mut orbit, mut transform) in camera_query.iter_mut() {
                *orbit = OrbitCamera::default();
                transform.translation = Vec3::new(-5.0, 5.0, 5.0);
                transform.look_at(Vec3::ZERO, Vec3::Y);
            }
        }
    });
}

fn update_mesh_system(
    mut params: ResMut<CadParams>,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<&Mesh3d, With<CadModel>>,
) {
    if !params.regenerate {
        return;
    }
    
    // 1. Create geometry with truck
    // circle on XZ plane
    let vertex = builder::vertex(Point3::new(params.radius, 0.0, 0.0));
    let circle_wire = builder::rsweep(&vertex, Point3::origin(), Vector3::unit_y(), Rad(std::f64::consts::TAU));
    let face = builder::try_attach_plane(&[circle_wire]).unwrap();
    let solid = builder::tsweep(&face, Vector3::new(0.0, params.height, 0.0));

    // 2. Tessellate
    let mesh_truck: PolygonMesh = solid.triangulation(params.resolution).to_polygon();

    // 3. Convert to Bevy Mesh
    let mut bevy_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()); 

    let mut combined_positions = Vec::new();
    let mut combined_normals = Vec::new();
    let mut combined_indices = Vec::new();
    let mut index_counter = 0;

    // Iterate over faces (triangles usually after triangulation)
    let faces = mesh_truck.faces();
    for i in 0..faces.len() {
        let face = &faces[i];
        for k in 0..3 {
            let v_idx = face[k].pos;
            let n_idx = face[k].nor; // Might be None if no normals, but we expect them.

            if let Some(n_idx) = n_idx {
                let p = mesh_truck.positions()[v_idx];
                let n = mesh_truck.normals()[n_idx];
                
                combined_positions.push([p.x as f32, p.y as f32, p.z as f32]);
                combined_normals.push([n.x as f32, n.y as f32, n.z as f32]);
                combined_indices.push(index_counter);
                index_counter += 1;
            }
        }
    }

    bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, combined_positions);
    bevy_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, combined_normals);
    bevy_mesh.insert_indices(Indices::U32(combined_indices));

    // 4. Update Bevy Resource
    if let Some(handle) = query.iter().next() {
        if let Some(mesh) = meshes.get_mut(handle) {
            *mesh = bevy_mesh;
        }
    }

    params.regenerate = false;
}
