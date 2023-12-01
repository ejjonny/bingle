use bevy::{
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
    utils::{HashMap, HashSet},
    window::{PresentMode, PrimaryWindow, WindowTheme},
};
use bevy_rapier2d::prelude::*;
use bevy_turborand::prelude::*;

const BUCKET_WIDTH: f32 = 300.;
const BUCKET_HEIGHT: f32 = 180.;
const BUCKET_Y_OFFSET: f32 = -100.;
const UPCOMING_BALL_POSITION: Vec3 =
    Vec3::new(-BUCKET_WIDTH * 0.5 - BARRIER_PADDING * 0.5, 0., 0.);
const BARRIER_PADDING: f32 = 100.;
const STRIKE_LIMIT: i32 = 4;
const COLOR_CYCLE_COUNT: i32 = 6;
const GROW_DURATION_SECONDS: f32 = 2.;
const DROPPABLE_RANGE: i32 = 4;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "b i n g l e".into(),
                    resolution: (
                        BUCKET_WIDTH.max(BUCKET_HEIGHT) + BARRIER_PADDING * 2.,
                        BUCKET_WIDTH.max(BUCKET_HEIGHT) + BARRIER_PADDING * 2.,
                    )
                        .into(),
                    present_mode: PresentMode::AutoVsync,
                    // Tells wasm to resize the window according to the available canvas
                    fit_canvas_to_parent: true,
                    // Tells wasm not to override default event handling, like F5, Ctrl+R etc.
                    prevent_default_event_handling: false,
                    window_theme: Some(WindowTheme::Dark),
                    enabled_buttons: bevy::window::EnabledButtons {
                        maximize: false,
                        ..Default::default()
                    },
                    ..default()
                }),
                ..default()
            }),
        )
        // .add_plugins(DefaultPlugins)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        // .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(RngPlugin::default())
        .add_systems(Startup, (setup_dropper, setup_graphics, setup_physics))
        .add_event::<GameOverEvent>()
        .add_event::<RestartGameEvent>()
        .add_systems(
            Update,
            (
                my_cursor_system,
                mouse_click_system,
                squash_balls,
                grow_system,
                check_game_state,
                text_update_system,
                update_score_system,
                game_over_system,
                restart_game_system,
            ),
        )
        .run();
}

#[derive(Resource, Default)]
struct CursorWorldPosition(Vec2);

#[derive(Resource)]
struct Game {
    dropper: Dropper,
    strikes: i32,
    over: bool,
    interpolated_score: i32,
    score: i32,
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct Dropper {
    rng: RngComponent,
    next_ball: Ball,
    mesh: Entity,
}

#[derive(Component)]
struct Ball {
    ball_type: BallType,
}

#[derive(Component)]
struct OutOfBoundsBarrier;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct StrikeText;

#[derive(Component)]
struct GameOverlay;

#[derive(Component)]
struct GameOverOverlay;

#[derive(Event)]
struct GameOverEvent;

#[derive(Event)]
struct RestartGameEvent;

#[derive(Component)]
struct BallProgress(f32);

#[derive(Component)]
struct BallTarget(i32);

fn setup_dropper(
    mut commands: Commands,
    mut global_rng: ResMut<GlobalRng>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = RngComponent::from(&mut global_rng);
    let first_ball = BallType::from_i32(rng.i32(1..=DROPPABLE_RANGE));
    let mesh = commands
        .spawn(first_ball.mesh(true, None, &mut meshes, &mut materials))
        .id();
    commands.insert_resource(Game {
        dropper: Dropper {
            rng,
            next_ball: Ball {
                ball_type: first_ball,
            },
            mesh,
        },
        strikes: 0,
        over: false,
        interpolated_score: 0,
        score: 0,
    });
}

fn setup_graphics(mut commands: Commands, mut game_ev: EventWriter<RestartGameEvent>) {
    commands.init_resource::<CursorWorldPosition>();
    commands.spawn((Camera2dBundle::default(), MainCamera));
    game_ev.send(RestartGameEvent {});
}

fn spawn_walls(
    commands: &mut Commands,
    walls: &mut Vec<(f32, f32, f32, f32)>,
    barrier: bool,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    for wall in walls.iter() {
        let width = wall.0;
        let height = wall.1;
        let x = wall.2;
        let y = wall.3;

        if barrier {
            commands
                .spawn((
                    MaterialMesh2dBundle {
                        mesh: meshes.add(shape::Box::new(width, height, 0.).into()).into(),
                        material: materials.add(ColorMaterial::from(Color::RED)),
                        transform: Transform::IDENTITY,
                        ..default()
                    },
                    OutOfBoundsBarrier,
                ))
                .insert(Collider::cuboid(width / 2., height / 2.))
                .insert(TransformBundle::from(Transform::from_xyz(x, y, 0.0)));
        } else {
            commands
                .spawn(MaterialMesh2dBundle {
                    mesh: meshes.add(shape::Box::new(width, height, 0.).into()).into(),
                    material: materials.add(ColorMaterial::from(Color::ANTIQUE_WHITE)),
                    transform: Transform::IDENTITY,
                    ..default()
                })
                .insert(Collider::cuboid(width / 2., height / 2.))
                .insert(TransformBundle::from(Transform::from_xyz(x, y, 0.0)));
        }
    }
}

fn setup_physics(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut walls = Vec::<(f32, f32, f32, f32)>::new();
    // Floor
    walls.push((BUCKET_WIDTH + 20., 20., 0., -(BUCKET_HEIGHT / 2.) + BUCKET_Y_OFFSET));
    // Left wall
    walls.push((20., BUCKET_HEIGHT + 20., -(BUCKET_WIDTH / 2.), BUCKET_Y_OFFSET));
    // Right wall
    walls.push((20., BUCKET_HEIGHT + 20., BUCKET_WIDTH / 2., BUCKET_Y_OFFSET));
    spawn_walls(
        &mut commands,
        &mut walls,
        false,
        &mut meshes,
        &mut materials,
    );
    walls.clear();

    let largest_dimension = BUCKET_WIDTH.max(BUCKET_HEIGHT);
    // Left wall
    walls.push((
        20.,
        largest_dimension + BARRIER_PADDING * 2. + 20.,
        largest_dimension / 2. + BARRIER_PADDING,
        0.,
    ));
    walls.push((
        20.,
        largest_dimension + BARRIER_PADDING * 2. + 20.,
        -(largest_dimension / 2. + BARRIER_PADDING),
        0.,
    ));
    // Cieling
    walls.push((
        largest_dimension + BARRIER_PADDING * 2. + 20.,
        20.,
        0.,
        largest_dimension / 2. + BARRIER_PADDING,
    ));
    // Floor
    walls.push((
        largest_dimension + BARRIER_PADDING * 2. + 20.,
        20.,
        0.,
        -(largest_dimension / 2. + BARRIER_PADDING),
    ));
    spawn_walls(&mut commands, &mut walls, true, &mut meshes, &mut materials);
}

fn mouse_click_system(
    mut commands: Commands,
    mouse_button: Res<Input<MouseButton>>,
    mouse_pos: Res<CursorWorldPosition>,
    existing_balls: Query<(Entity, &BallType, &Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut game: ResMut<Game>,
    mut game_ev: EventWriter<RestartGameEvent>,
) {
    if mouse_button.just_released(MouseButton::Left) {
        if !game.over {
            let dropper = &mut game.dropper;
            let current_ball_type = dropper.next_ball.ball_type;
            let position = mouse_pos.0.x.clamp(-BUCKET_WIDTH, BUCKET_WIDTH);
            let blocked = existing_balls
                .iter()
                .any(|(_, _, transform)| transform.translation.y >= 100.0 && position - transform.translation.x < 35.);
            if !blocked {
                spawn_ball(
                    &mut commands,
                    current_ball_type,
                    None,
                    Transform::from_xyz(position, 190.0, 0.0),
                    &mut meshes,
                    &mut materials,
                );
                let new_ball = BallType::from_i32(dropper.rng.i32(1..=DROPPABLE_RANGE));
                game.dropper.next_ball.ball_type = new_ball;
                // Swap upcoming mesh
                commands.get_entity(game.dropper.mesh).unwrap().despawn();
                game.dropper.mesh = commands
                    .spawn(new_ball.mesh(true, None, &mut meshes, &mut materials))
                    .id()
            }
        } else {
            game_ev.send(RestartGameEvent {});
        }
    }
}

fn spawn_ball(
    commands: &mut Commands,
    current_ball_type: BallType,
    target_ball_type: Option<BallTarget>,
    position: Transform,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let mut ball;
    if let Some(target) = target_ball_type {
        ball = commands.spawn((
            current_ball_type.mesh(
                false,
                Some(BallType::Simple(target.0).color()),
                meshes,
                materials,
            ),
            current_ball_type,
            target,
            BallProgress(0.),
        ));
    } else {
        ball = commands.spawn((
            current_ball_type.mesh(false, None, meshes, materials),
            current_ball_type,
        ));
    }
    ball.insert(RigidBody::Dynamic)
        .insert(Collider::ball(current_ball_type.size()))
        .insert(Restitution::coefficient(0.2))
        .insert(GravityScale(4.))
        .insert(Velocity::linear(Vect::new(0.0, -100.0)))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(TransformBundle::from(position));
}

fn grow_system(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut balls_growing: Query<(
        Entity,
        &mut BallType,
        &BallTarget,
        &mut BallProgress,
        &mut Collider,
        &Mesh2dHandle,
    )>,
) {
    for (_, mut ball_type, target, mut progress, mut collider, mesh) in balls_growing.iter_mut() {
        progress.0 += time.delta_seconds() / GROW_DURATION_SECONDS;
        if progress.0 >= 1. {
            *ball_type = BallType::Simple(target.0);
            let size = ball_type.size();
            *collider = Collider::ball(size);
            if let Some(mesh) = meshes.get_mut(&mesh.0) {
                *mesh = shape::Circle::new(size).into();
            }
        } else {
            let from = ball_type.size();
            let to = BallType::Simple(target.0).size();
            let size = from + ((to - from) * progress.0);
            *collider = Collider::ball(size);
            if let Some(mesh) = meshes.get_mut(&mesh.0) {
                *mesh = shape::Circle::new(size).into();
            }
        }
    }
}

fn check_game_state(mut game: ResMut<Game>, mut game_ev: EventWriter<GameOverEvent>) {
    if game.strikes >= STRIKE_LIMIT && !game.over {
        game.over = true;
        game_ev.send(GameOverEvent {});
    }
}

fn update_score_system(mut game: ResMut<Game>) {
    if game.score - game.interpolated_score >= 10 {
        game.interpolated_score += 10;
    } else {
        game.interpolated_score = game.score;
    }
}

#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum BallType {
    Simple(i32),
    Special,
}

impl BallType {
    fn mesh(
        self,
        preview: bool,
        target_color: Option<ColorMaterial>,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<ColorMaterial>>,
    ) -> MaterialMesh2dBundle<ColorMaterial> {
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(self.size()).into()).into(),
            material: materials.add(if let Some(target_color) = target_color {
                target_color
            } else {
                self.color()
            }),
            transform: Transform::from_translation(if preview {
                UPCOMING_BALL_POSITION
            } else {
                Vec3::new(0., 0., 0.)
            }),
            ..default()
        }
    }
}

impl BallType {
    fn size(&self) -> f32 {
        match self {
            Self::Simple(size) => return 7. + *size as f32 * 7.,
            Self::Special => return 10.,
        }
    }
}

impl BallType {
    fn color(self) -> ColorMaterial {
        let sequence = vec![
            Color::ORANGE,
            Color::GRAY,
            Color::SEA_GREEN,
            Color::YELLOW_GREEN,
            Color::YELLOW,
            Color::GOLD,
        ];
        assert!(sequence.len() as i32 == COLOR_CYCLE_COUNT);
        match self {
            Self::Simple(size) => {
                return ColorMaterial::from(*sequence.get(size as usize % sequence.len()).unwrap())
            }
            Self::Special => return ColorMaterial::from(Color::BLACK),
        }
    }
}

impl BallType {
    fn from_i32(value: i32) -> BallType {
        if value <= 5 {
            return BallType::Simple(value);
        } else {
            return BallType::Special;
        }
    }
}

fn my_cursor_system(
    mut mycoords: ResMut<CursorWorldPosition>,
    // query to get the window (so we can read the current cursor position)
    q_window: Query<&Window, With<PrimaryWindow>>,
    // query to get camera transform
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    // get the camera info and transform
    // assuming there is exactly one main camera entity, so Query::single() is OK
    let (camera, camera_transform) = q_camera.single();

    // There is only one primary window, so we can similarly get it from the query:
    let window = q_window.single();

    // check if the cursor is inside the window and get its position
    // then, ask bevy to convert into world coordinates, and truncate to discard Z
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
        .map(|ray| ray.origin.truncate())
    {
        mycoords.0 = world_position;
    }
}

fn squash_balls(
    mut game: ResMut<Game>,
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    balls: Query<(Entity, &BallType, Option<&BallTarget>, &Transform)>,
    barriers: Query<(Entity, &OutOfBoundsBarrier)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut contacts = HashSet::new();
    let mut ball_types = HashMap::<Entity, (BallType, Transform)>::new();
    for collision_event in collision_events.read() {
        match collision_event {
            CollisionEvent::Started(entity_a, entity_b, _) => {
                contacts.insert((entity_a, entity_b));
            }
            _ => (),
        }
    }
    for (entity, ball_type, ball_target, transform) in balls.iter() {
        let match_type;
        if let Some(target) = ball_target {
            match_type = BallType::Simple(target.0);
        } else {
            match_type = *ball_type;
        }
        ball_types.insert(entity, (match_type, *transform));
    }
    let mut barrier_entities = HashMap::<Entity, bool>::new();
    for (entity, _) in barriers.iter() {
        barrier_entities.insert(entity, true);
    }
    for contact in contacts {
        match (ball_types.get(contact.0), ball_types.get(contact.1)) {
            (
                Some((BallType::Simple(level_a), transform_a)),
                Some((BallType::Simple(level_b), transform_b)),
            ) => {
                // if level_a % COLOR_CYCLE_COUNT == level_b % COLOR_CYCLE_COUNT {
                if level_a == level_b {
                    // let middle = transform_a.translation.lerp(transform_b.translation, 0.5);
                    commands.entity(*contact.0).despawn();
                    commands.entity(*contact.1).despawn();
                    let larger = i32::max(*level_a, *level_b);
                    let a_larger = larger == *level_a;
                    spawn_ball(
                        &mut commands,
                        BallType::Simple(larger),
                        Some(BallTarget(larger + 1)),
                        if a_larger { *transform_a } else { *transform_b },
                        &mut meshes,
                        &mut materials,
                    );
                    game.score += (level_a + level_b) * 11;
                }
            }
            _ => {
                let mut hit_barrier = false;
                if barrier_entities.get(contact.0) == Some(&true) {
                    commands.get_entity(*contact.1).unwrap();
                    commands.entity(*contact.1).despawn();
                    hit_barrier = true;
                } else if barrier_entities.get(contact.1) == Some(&true) {
                    commands.get_entity(*contact.0).unwrap();
                    commands.entity(*contact.0).despawn();
                    hit_barrier = true;
                }
                if hit_barrier {
                    game.strikes += 1;
                }
            }
        }
    }
}

fn text_update_system(
    game: ResMut<Game>,
    mut score_text: Query<&mut Text, With<ScoreText>>,
    mut strike_text: Query<&mut Text, (With<StrikeText>, Without<ScoreText>)>,
) {
    for mut text in &mut score_text {
        let score = game.interpolated_score;
        text.sections[0].value = format!("{score}");
    }
    for mut text in &mut strike_text {
        let strikes = STRIKE_LIMIT - game.strikes;
        text.sections[0].value = format!("{strikes}/{STRIKE_LIMIT}")
    }
}

fn restart_game_system(
    mut game: ResMut<Game>,
    mut commands: Commands,
    overlay: Query<Entity, With<GameOverOverlay>>,
    balls: Query<Entity, With<BallType>>,
    asset_server: Res<AssetServer>,
    game_ev: EventReader<RestartGameEvent>,
) {
    if !game_ev.is_empty() {
        game.score = 0;
        game.strikes = 0;
        game.over = false;
        for entity in overlay.iter() {
            commands.entity(entity).despawn();
        }
        for entity in balls.iter() {
            commands.entity(entity).despawn();
        }
        commands
            .spawn((
                NodeBundle {
                    style: Style {
                        // fill the entire window
                        width: Val::Percent(100.),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    background_color: BackgroundColor(Color::Rgba {
                        red: 0.,
                        green: 0.,
                        blue: 0.,
                        alpha: 0.5,
                    }),
                    ..Default::default()
                },
                GameOverlay,
            ))
            .with_children(|builder| {
                builder.spawn((
                    TextBundle::from_section(
                        "0",
                        TextStyle {
                            font: asset_server.load("fonts/kuga.ttf"),
                            font_size: 80.0,
                            ..default()
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexStart,
                        justify_content: JustifyContent::Center,
                        ..default()
                    }),
                    ScoreText,
                    GameOverlay,
                ));
                builder.spawn((
                    TextBundle::from_section(
                        format!("{STRIKE_LIMIT}/{STRIKE_LIMIT}"),
                        TextStyle {
                            font: asset_server.load("fonts/kuga.ttf"),
                            font_size: 30.0,
                            color: Color::RED,
                            ..default()
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexStart,
                        justify_content: JustifyContent::FlexEnd,
                        ..default()
                    }),
                    StrikeText,
                    GameOverlay,
                ));
            });
    }
}

fn game_over_system(
    game: ResMut<Game>,
    mut commands: Commands,
    overlay: Query<Entity, With<GameOverlay>>,
    asset_server: Res<AssetServer>,
    game_ev: EventReader<GameOverEvent>,
) {
    if !game_ev.is_empty() {
        for entity in overlay.iter() {
            commands.entity(entity).despawn();
        }
        commands
            .spawn((
                NodeBundle {
                    style: Style {
                        // fill the entire window
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..Default::default()
                    },
                    background_color: BackgroundColor(Color::BLACK),
                    ..Default::default()
                },
                GameOverOverlay,
            ))
            .with_children(|builder| {
                builder.spawn((
                    TextBundle::from_section(
                        "Game Over...",
                        TextStyle {
                            font: asset_server.load("fonts/kuga.ttf"),
                            font_size: 100.0,
                            ..default()
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    }),
                    GameOverOverlay,
                ));
                let score = game.score;
                builder.spawn((
                    TextBundle::from_section(
                        format!("High score: {score}"),
                        TextStyle {
                            font: asset_server.load("fonts/kuga.ttf"),
                            font_size: 70.0,
                            ..default()
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    }),
                    GameOverOverlay,
                ));
                builder.spawn((
                    TextBundle::from_section(
                        "Click anywhere to restart",
                        TextStyle {
                            font: asset_server.load("fonts/kuga.ttf"),
                            font_size: 30.0,
                            ..default()
                        },
                    )
                    .with_text_alignment(TextAlignment::Center)
                    .with_style(Style {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    }),
                    GameOverOverlay,
                ));
            });
    }
}
