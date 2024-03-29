use bevy::{
    input::touch::TouchPhase,
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
    utils::{HashMap, HashSet},
    window::{PrimaryWindow, WindowTheme},
};
use bevy_rapier2d::prelude::*;
use bevy_turborand::prelude::*;

const UNIVERSAL_SCALE: f32 = 1.;
const BUCKET_WIDTH: f32 = 300. * UNIVERSAL_SCALE;
const BUCKET_HEIGHT: f32 = 150. * UNIVERSAL_SCALE;
const BUCKET_Y_OFFSET: f32 = -100. * UNIVERSAL_SCALE;
const UPCOMING_BALL_POSITION: Vec3 = Vec3::new(-BUCKET_WIDTH * 0.5 - BARRIER_PADDING * 0.5, 0., 0.);
const BARRIER_PADDING: f32 = 100. * UNIVERSAL_SCALE;
const STRIKE_LIMIT: i32 = 4;
const COLOR_CYCLE_COUNT: i32 = 6;
const GROW_DURATION_SECONDS: f32 = 2.;
const DROPPABLE_RANGE: i32 = 4;
const BALL_BASE_SIZE: f32 = 7. * UNIVERSAL_SCALE;
const BALL_LEVEL_SIZE: f32 = 7. * UNIVERSAL_SCALE;
const WALL_THICKNESS: f32 = 20. * UNIVERSAL_SCALE;
const BALL_DROPPER_OFFSET: f32 = 190. * UNIVERSAL_SCALE;
const DROP_SPAM_Y_BLOCK_OFFSET: f32 = 100. * UNIVERSAL_SCALE;
const DROP_SPAM_X_BLOCK_DISTANCE: f32 = 35. * UNIVERSAL_SCALE;

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
        .add_plugins(RapierPhysicsPlugin::<()>::default().in_schedule(FixedUpdate))
        // .add_plugins(RapierDebugRenderPlugin::default())
        .add_plugins(RngPlugin::default())
        .add_systems(Startup, (setup_dropper, setup_graphics, setup_physics))
        .add_event::<GameOverEvent>()
        .add_event::<RestartGameEvent>()
        .add_systems(
            Update,
            (
                my_cursor_system,
                mouse_click_system
                    .after(my_cursor_system),
                touch_events_system
                    .after(my_cursor_system),
                collision_system
                    .after(touch_events_system),
                squash_balls
                    .after(collision_system),
                grow_system
                    .after(squash_balls),
                game_over_system,
                restart_game_system
            ),
        )
        .add_systems(PostUpdate, (check_game_state, update_score_system, text_update_system))
        .run();
}

#[derive(Resource, Default)]
struct CursorWorldPosition(Vec2);

#[derive(Resource, Default)]
struct TouchWorldPosition(Vec2);

#[derive(Resource, Default)]
struct Contacts(HashSet<(Entity, Entity)>);

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
    let mut config = RapierConfiguration::default();
    config.timestep_mode = TimestepMode::Fixed { dt: 0.03, substeps: 2 };
    commands.insert_resource(config);
    commands.insert_resource(Contacts(HashSet::<(Entity, Entity)>::new()));
    let mut walls = Vec::<(f32, f32, f32, f32)>::new();
    // Floor
    walls.push((
        BUCKET_WIDTH + WALL_THICKNESS,
        WALL_THICKNESS,
        0.,
        -(BUCKET_HEIGHT / 2.) + BUCKET_Y_OFFSET,
    ));
    // Left wall
    walls.push((
        WALL_THICKNESS,
        BUCKET_HEIGHT + WALL_THICKNESS,
        -(BUCKET_WIDTH / 2.),
        BUCKET_Y_OFFSET,
    ));
    // Right wall
    walls.push((
        WALL_THICKNESS,
        BUCKET_HEIGHT + WALL_THICKNESS,
        BUCKET_WIDTH / 2.,
        BUCKET_Y_OFFSET,
    ));
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
        WALL_THICKNESS,
        largest_dimension + BARRIER_PADDING * 2. + WALL_THICKNESS,
        largest_dimension / 2. + BARRIER_PADDING,
        0.,
    ));
    walls.push((
        WALL_THICKNESS,
        largest_dimension + BARRIER_PADDING * 2. + WALL_THICKNESS,
        -(largest_dimension / 2. + BARRIER_PADDING),
        0.,
    ));
    // Cieling
    walls.push((
        largest_dimension + BARRIER_PADDING * 2. + WALL_THICKNESS,
        WALL_THICKNESS,
        0.,
        largest_dimension / 2. + BARRIER_PADDING,
    ));
    // Floor
    walls.push((
        largest_dimension + BARRIER_PADDING * 2. + WALL_THICKNESS,
        WALL_THICKNESS,
        0.,
        -(largest_dimension / 2. + BARRIER_PADDING),
    ));
    spawn_walls(&mut commands, &mut walls, true, &mut meshes, &mut materials);
}

fn touch_events_system(
    mut touch_evr: EventReader<TouchInput>,
    commands: Commands,
    existing_balls: Query<(Entity, &BallType, &Transform)>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    game: ResMut<Game>,
    game_ev: EventWriter<RestartGameEvent>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    if let Some(event) = touch_evr.read().last() {
        match event.phase {
            TouchPhase::Ended => {
                let (camera, camera_transform) = q_camera.single();
                if let Some(world_position) = camera
                    .viewport_to_world(camera_transform, event.position)
                    .map(|ray| ray.origin.truncate())
                {
                    click(
                        commands,
                        existing_balls,
                        world_position,
                        meshes,
                        materials,
                        game,
                        game_ev,
                    );
                }
            }
            _ => (),
        }
    }
}

fn mouse_click_system(
    commands: Commands,
    mouse_button: Res<Input<MouseButton>>,
    mouse_pos: Res<CursorWorldPosition>,
    existing_balls: Query<(Entity, &BallType, &Transform)>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    game: ResMut<Game>,
    game_ev: EventWriter<RestartGameEvent>,
) {
    if mouse_button.just_released(MouseButton::Left) {
        click(
            commands,
            existing_balls,
            mouse_pos.0,
            meshes,
            materials,
            game,
            game_ev,
        );
    }
}

fn click(
    mut commands: Commands,
    existing_balls: Query<(Entity, &BallType, &Transform)>,
    click_position: Vec2,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut game: ResMut<Game>,
    mut game_ev: EventWriter<RestartGameEvent>,
) {
    if !game.over {
        let dropper = &mut game.dropper;
        let current_ball_type = dropper.next_ball.ball_type;
        let position = click_position.x.clamp(
            -BUCKET_WIDTH * 0.5 - (BARRIER_PADDING * 0.5),
            BUCKET_WIDTH * 0.5 + (BARRIER_PADDING * 0.5),
        );
        let blocked = existing_balls.iter().any(|(_, _, transform)| {
            transform.translation.y >= DROP_SPAM_Y_BLOCK_OFFSET
                && position - transform.translation.x < DROP_SPAM_X_BLOCK_DISTANCE
        });
        if !blocked {
            spawn_ball(
                &mut commands,
                current_ball_type,
                None,
                Transform::from_xyz(position, BALL_DROPPER_OFFSET, 0.0),
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
        .insert(Friction::coefficient(0.))
        .insert(GravityScale(4.))
        .insert(Velocity::linear(Vect::new(0.0, -0.0)))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(TransformBundle::from(position));
}

fn grow_system(
    mut commands: Commands,
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
    for (entity, mut ball_type, target, mut progress, mut collider, mesh) in
        balls_growing.iter_mut()
    {
        progress.0 += time.delta_seconds() / GROW_DURATION_SECONDS;
        if progress.0 >= 1. {
            *ball_type = BallType::Simple(target.0);
            commands.entity(entity).remove::<BallProgress>();
            commands.entity(entity).remove::<BallTarget>();
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
            Self::Simple(size) => return BALL_BASE_SIZE + *size as f32 * BALL_LEVEL_SIZE,
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

fn collision_system(
    mut collision_events: EventReader<CollisionEvent>,
    mut contacts: ResMut<Contacts>,
) {
    for collision_event in collision_events.read() {
        match collision_event {
            CollisionEvent::Started(entity_a, entity_b, _) => {
                contacts.0.insert((*entity_a, *entity_b));
            }
            CollisionEvent::Stopped(entity_a, entity_b, _) => {
                contacts.0.remove(&(*entity_a, *entity_b));
            }
        }
    }
}

fn squash_balls(
    mut game: ResMut<Game>,
    mut commands: Commands,
    mut contacts: ResMut<Contacts>,
    balls: Query<(
        Entity,
        &BallType,
        Option<&BallTarget>,
        Option<&BallProgress>,
        &Transform,
        &Handle<ColorMaterial>,
    )>,
    barriers: Query<(Entity, &OutOfBoundsBarrier)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut ball_types = HashMap::<Entity, (BallType, Transform)>::new();
    for (entity, ball_type, ball_target, _, transform, _) in balls.iter() {
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
    let mut to_remove = HashSet::<(Entity, Entity)>::new();
    let mut visited = HashSet::<Entity>::new();
    for contact in contacts.0.iter() {
        match (ball_types.get(&contact.0), ball_types.get(&contact.1)) {
            (
                Some((BallType::Simple(level_a), transform_a)),
                Some((BallType::Simple(level_b), transform_b)),
            ) => {
                // if level_a % COLOR_CYCLE_COUNT == level_b % COLOR_CYCLE_COUNT {
                if level_a == level_b {
                    let lower = f32::min(transform_a.translation.y, transform_b.translation.y);
                    let a_lower = lower == transform_a.translation.y;
                    let replaced = if a_lower { contact.0 } else { contact.1 };
                    let removed = if a_lower { contact.1 } else { contact.0 };
                    if visited.insert(replaced) {
                        commands.entity(removed).despawn();
                        if let Some(replaced_ball) = balls.iter().find(|ball| ball.0 == replaced) {
                            // Update existing entity's color & add components for growth
                            let upgraded_ball_type = BallType::Simple(level_a + 1);
                            materials.insert(
                                replaced_ball.5,
                                ColorMaterial::from(upgraded_ball_type.color()),
                            );
                            commands.entity(replaced).insert(BallTarget(level_a + 1));
                            if let Some(current_progress) = replaced_ball.3 {
                                commands
                                    .entity(replaced)
                                    .insert(BallProgress(current_progress.0 * 0.5));
                            } else {
                                commands.entity(replaced).insert(BallProgress(0.));
                            }
                            to_remove.insert(*contact);
                        }
                        game.score += (level_a + level_b) * 11;
                    }
                }
            }
            _ => {
                let mut hit_barrier = false;
                if barrier_entities.get(&contact.0) == Some(&true) {
                    commands.get_entity(contact.1).unwrap();
                    commands.entity(contact.1).despawn();
                    hit_barrier = true;
                } else if barrier_entities.get(&contact.1) == Some(&true) {
                    commands.get_entity(contact.0).unwrap();
                    commands.entity(contact.0).despawn();
                    hit_barrier = true;
                }
                if hit_barrier {
                    to_remove.insert(*contact);
                    game.strikes += 1;
                }
            }
        }
    }
    for despawned in to_remove {
        contacts.0.remove(&despawned);
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
    mut game_ev: EventReader<RestartGameEvent>,
    mut contacts: ResMut<Contacts>,
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
        contacts.0.drain();
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
                        top: Val::Px(10.),
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
    game_ev.clear();
}

fn game_over_system(
    game: ResMut<Game>,
    mut commands: Commands,
    overlay: Query<Entity, With<GameOverlay>>,
    asset_server: Res<AssetServer>,
    mut game_ev: EventReader<GameOverEvent>,
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
    game_ev.clear();
}
