use bevy::{
    prelude::*,
    sprite::MaterialMesh2dBundle,
    utils::{HashMap, HashSet},
    window::PrimaryWindow,
};
use bevy_rapier2d::prelude::*;
use bevy_turborand::prelude::*;

const BUCKET_WIDTH: f32 = 200.;
const BUCKET_HEIGHT: f32 = 200.;
const UPCOMING_BALL_POSITION: Vec3 = Vec3::new(BUCKET_WIDTH * 0.5 + 100., 0., 0.);
const BARRIER_PADDING: f32 = 200.;
const STRIKE_LIMIT: i32 = 4;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
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

fn setup_dropper(
    mut commands: Commands,
    mut global_rng: ResMut<GlobalRng>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut rng = RngComponent::from(&mut global_rng);
    let first_ball = BallType::from_i32(rng.i32(1..=5));
    let mesh = commands
        .spawn(first_ball.mesh(true, &mut meshes, &mut materials))
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

fn setup_graphics(
    mut commands: Commands,
    mut game_ev: EventWriter<RestartGameEvent>,
) {
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
    walls.push((BUCKET_WIDTH + 20., 20., 0., -(BUCKET_HEIGHT / 2.)));
    // Left wall
    walls.push((20., BUCKET_HEIGHT + 20., -(BUCKET_WIDTH / 2.), 0.));
    // Right wall
    walls.push((20., BUCKET_HEIGHT + 20., BUCKET_WIDTH / 2., 0.));
    spawn_walls(&mut commands, &mut walls, false, &mut meshes, &mut materials);
    walls.clear();

    // Left wall
    walls.push((
        20.,
        BUCKET_HEIGHT + BARRIER_PADDING * 2. + 20.,
        BUCKET_WIDTH / 2. + BARRIER_PADDING,
        0.,
    ));
    walls.push((
        20.,
        BUCKET_HEIGHT + BARRIER_PADDING * 2. + 20.,
        -(BUCKET_WIDTH / 2. + BARRIER_PADDING),
        0.,
    ));
    // Cieling
    walls.push((
        BUCKET_WIDTH + BARRIER_PADDING * 2. + 20.,
        20.,
        0.,
        BUCKET_HEIGHT / 2. + BARRIER_PADDING,
    ));
    // Floor
    walls.push((
        BUCKET_WIDTH + BARRIER_PADDING * 2. + 20.,
        20.,
        0.,
        -(BUCKET_HEIGHT / 2. + BARRIER_PADDING),
    ));
    spawn_walls(&mut commands, &mut walls, true, &mut meshes, &mut materials);
}

fn mouse_click_system(
    mut commands: Commands,
    mouse_button: Res<Input<MouseButton>>,
    mouse_pos: Res<CursorWorldPosition>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut game: ResMut<Game>,
    mut game_ev: EventWriter<RestartGameEvent>,
) {
    if mouse_button.just_released(MouseButton::Left) {
        if !game.over {
            let dropper = &mut game.dropper;
            let current_ball_type = dropper.next_ball.ball_type;
            let position = mouse_pos.0.x;
            spawn_ball(
                &mut commands,
                current_ball_type,
                Transform::from_xyz(position, 200.0, 0.0),
                &mut meshes,
                &mut materials,
            );
            let new_ball = BallType::from_i32(dropper.rng.i32(1..=5));
            game.dropper.next_ball.ball_type = new_ball;
            // Swap upcoming mesh
            commands.get_entity(game.dropper.mesh).unwrap().despawn();
            game.dropper.mesh = commands
                .spawn(new_ball.mesh(true, &mut meshes, &mut materials))
                .id()
        } else {
            game_ev.send(RestartGameEvent {});
        }
    }
}

fn spawn_ball(
    commands: &mut Commands,
    ball_type: BallType,
    position: Transform,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    commands
        .spawn((ball_type.mesh(false, meshes, materials), ball_type))
        .insert(RigidBody::Dynamic)
        .insert(Collider::ball(ball_type.size()))
        .insert(Restitution::coefficient(0.1))
        .insert(Velocity::linear(Vect::new(0.0, -200.0)))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(TransformBundle::from(position));
}

fn check_game_state(mut game: ResMut<Game>, mut game_ev: EventWriter<GameOverEvent>) {
    if game.strikes >= STRIKE_LIMIT && !game.over {
        game.over = true;
        game_ev.send(GameOverEvent {});
    }
}


fn update_score_system(
    mut game: ResMut<Game>,
    time: Res<Time>,
) {
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
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<ColorMaterial>>,
    ) -> MaterialMesh2dBundle<ColorMaterial> {
        MaterialMesh2dBundle {
            mesh: meshes.add(shape::Circle::new(self.size()).into()).into(),
            material: materials.add(self.color()),
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
            Self::Simple(size) => return 10. + *size as f32 * 4.,
            Self::Special => return 10.,
        }
    }
}

impl BallType {
    fn color(self) -> ColorMaterial {
        let sequence = vec![
            Color::ALICE_BLUE,
            Color::DARK_GRAY,
            Color::SEA_GREEN,
            Color::YELLOW_GREEN,
            Color::ORANGE_RED,
            Color::PURPLE,
        ];
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
    balls: Query<(Entity, &BallType, &Transform)>,
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
    for (entity, ball_type, transform) in balls.iter() {
        ball_types.insert(entity, (*ball_type, *transform));
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
                if level_a == level_b {
                    let middle = transform_a.translation.lerp(transform_b.translation, 0.5);
                    spawn_ball(
                        &mut commands,
                        BallType::Simple(level_a + 1),
                        Transform::from_translation(middle),
                        &mut meshes,
                        &mut materials,
                    );
                    game.score += (level_a + level_b) * 11;
                    commands.entity(*contact.0).despawn();
                    commands.entity(*contact.1).despawn();
                }
            }
            _ => (),
        }
        let mut hit_barrier = false;
        if barrier_entities.get(contact.0) == Some(&true) {
            commands.entity(*contact.1).despawn();
            hit_barrier = true;
        } else if barrier_entities.get(contact.1) == Some(&true) {
            commands.entity(*contact.0).despawn();
            hit_barrier = true;
        }
        if hit_barrier {
            game.strikes += 1;
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
                    background_color: BackgroundColor(Color::BLACK),
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
