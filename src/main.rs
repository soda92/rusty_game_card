use bevy::prelude::*;
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

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .init_resource::<CadParams>()
        .add_systems(Startup, setup)
        .add_systems(Update, (ui_system, update_mesh_system))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-5.0, 5.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
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

fn ui_system(mut contexts: EguiContexts, mut params: ResMut<CadParams>) {
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
        if ui.button("Regenerate").clicked() {
            params.regenerate = true;
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
