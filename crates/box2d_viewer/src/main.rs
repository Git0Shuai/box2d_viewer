#![windows_subsystem = "windows"]

mod uds;

use std::collections::HashMap;

use async_io::Async;
use futures_lite::{FutureExt, StreamExt};
use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Subscription, Theme, Vector,
    advanced::mouse::Click,
    futures::{
        SinkExt,
        channel::{
            mpsc::{Sender, channel},
            oneshot,
        },
    },
    mouse,
    widget::{
        self,
        canvas::{
            self, Path, Stroke,
            path::lyon_path::{
                geom::euclid::{Transform2D, Vector2D},
                math::Angle,
            },
        },
    },
    window,
};
use shared_types::{CircleShape, DrawMessage, RectShape, Shape, ShapeCodec};
use tokio_util::{codec::FramedRead, compat::FuturesAsyncReadCompatExt};
use uds::UnixListener;
use uds::UnixStream;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    iced::application("box2d-viewer", update, view)
        .subscription(subscription)
        .run()?;
    let _ = std::fs::remove_file("./view_box2d.sock");
    Ok(())
}

#[derive(Default)]
struct State {
    shutdown: Option<oneshot::Sender<()>>,
    canvases: HashMap<(i32, String), Box2dCanvasProg>,
}

#[derive(Default)]
struct Box2dCanvasProg {
    id: i32,
    name: String,
    shapes: Option<Vec<Shape>>,
    cache: canvas::Cache,
    active: bool,
}

struct CanvasState {
    pub left_click: Option<Click>,
    pub drag: bool,
    pub drag_cursor_last_pos: Option<iced::Point>,
    pub scale: f32,
    pub offset: iced::Vector,
}

impl Default for CanvasState {
    fn default() -> Self {
        CanvasState {
            left_click: None,
            drag: false,
            scale: 1_f32,
            drag_cursor_last_pos: None,
            offset: Default::default(),
        }
    }
}

impl canvas::Program<Message> for Box2dCanvasProg {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> (canvas::event::Status, Option<Message>) {
        let captured = canvas::event::Status::Captured;
        let redraw = Message::Redraw((self.id, self.name.clone()));

        match event {
            canvas::Event::Mouse(mouse_evt) => {
                log::error!("{:?}.", mouse_evt);
                match mouse_evt {
                    mouse::Event::WheelScrolled { delta } => {
                        let delta = match delta {
                            mouse::ScrollDelta::Lines { y, .. } => y,
                            mouse::ScrollDelta::Pixels { y, .. } => y,
                        };
                        let prev_scale = state.scale;
                        state.scale = (prev_scale * f32::powf(1.1, delta)).max(0.01).min(100.);
                        if let Some(pos) = cursor.position() {
                            // before: p_world * prev_scale + prev_offset = p_screen
                            // after:  p_world * state.scale + state.offset = p_screen
                            let p_screen = Vector::new(pos.x, pos.y);
                            state.offset = p_screen
                                - (p_screen - state.offset) * (1. / prev_scale) * state.scale
                        }
                        return (captured, Some(redraw));
                    }
                    mouse::Event::ButtonPressed(mouse::Button::Right) => {
                        state.drag = true;
                        state.drag_cursor_last_pos = cursor.position();
                        return (captured, None);
                    }
                    mouse::Event::ButtonPressed(mouse::Button::Left) => {
                        if let Some(pos) = cursor.position() {
                            let click = Click::new(pos, mouse::Button::Left, state.left_click);
                            if let iced::advanced::mouse::click::Kind::Double = click.kind() {
                                state.left_click = None;
                                if let Some(ref shapes) = self.shapes {
                                    let all_shapes_aabb: Option<(f32, f32, f32, f32)> =
                                        shapes.iter().fold(None, |acc, it| {
                                            let shape_aabb = match it {
                                                Shape::Circle(circle_shape) => (
                                                    circle_shape.pos.0 - circle_shape.radius,
                                                    circle_shape.pos.1 - circle_shape.radius,
                                                    circle_shape.pos.0 + circle_shape.radius,
                                                    circle_shape.pos.1 + circle_shape.radius,
                                                ),
                                                Shape::Rect(rect_shape) => (
                                                    rect_shape.top_left.0,
                                                    rect_shape.top_left.1 - rect_shape.size.1,
                                                    rect_shape.top_left.0 + rect_shape.size.0,
                                                    rect_shape.top_left.1,
                                                ),
                                            };
                                            if let Some(acc) = acc {
                                                Some((
                                                    acc.0.min(shape_aabb.0),
                                                    acc.1.min(shape_aabb.1),
                                                    acc.2.max(shape_aabb.2),
                                                    acc.3.max(shape_aabb.3),
                                                ))
                                            } else {
                                                Some(shape_aabb)
                                            }
                                        });
                                    if let Some(all_shapes_aabb) = all_shapes_aabb {
                                        let size = Size::new(
                                            all_shapes_aabb.2 - all_shapes_aabb.0,
                                            all_shapes_aabb.3 - all_shapes_aabb.1,
                                        );
                                        state.scale = (bounds.width / size.width)
                                            .min(bounds.height / size.height)
                                            .min(100.)
                                            .max(0.01);
                                        let center = Point::new(
                                            (all_shapes_aabb.0 + all_shapes_aabb.2) / 2.,
                                            -(all_shapes_aabb.1 + all_shapes_aabb.3) / 2.,
                                        ) * iced::Transformation::scale(state.scale);
                                        state.offset = bounds.center() - center;
                                        return (captured, Some(redraw));
                                    }
                                };
                            } else {
                                state.left_click = Some(click)
                            }
                        }
                        return (captured, None);
                    }
                    mouse::Event::CursorMoved { position } => {
                        if state.drag {
                            if let Some(last_pos) = state.drag_cursor_last_pos {
                                state.offset = state.offset + (position - last_pos);
                            }
                            state.drag_cursor_last_pos = Some(position);
                            return (captured, Some(redraw));
                        }
                    }
                    mouse::Event::ButtonReleased(mouse::Button::Right) => {
                        state.drag = false;
                        return (captured, None);
                    }
                    _ => {}
                }
            }
            _ => {}
        };

        (canvas::event::Status::Ignored, None)
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let Some(ref shapes) = self.shapes else {
                return;
            };
            let (scale, offset) = (state.scale, state.offset);
            for shape in shapes.iter() {
                match shape {
                    &Shape::Circle(CircleShape { pos, radius }) => {
                        let center = Point::ORIGIN + (Vector::new(pos.0, -pos.1) * scale) + offset;
                        frame.stroke(
                            &Path::circle(center, radius * state.scale),
                            Stroke::default(),
                        )
                    }
                    &Shape::Rect(RectShape {
                        top_left,
                        size,
                        rotate,
                    }) => {
                        let rect = Size::new(size.0, size.1) * scale;
                        let transform = Transform2D::rotation(Angle::radians(-rotate))
                            .then_translate(
                                Vector2D::new(top_left.0, -top_left.1) * scale
                                    + Vector2D::new(offset.x, offset.y),
                            );
                        frame.stroke(
                            &Path::rectangle(Point::ORIGIN, rect).transform(&transform),
                            Stroke::default(),
                        )
                    }
                }
            }
        });
        vec![geometry]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::default()
    }
}

#[derive(Debug)]
enum Message {
    IoReady(oneshot::Sender<()>),
    DebugDraw((i32, DrawMessage)),
    CloseConn(i32),
    Event(Event),
    Redraw((i32, String)),
    CheckActive,
}

fn update(state: &mut State, msg: Message) {
    match msg {
        Message::IoReady(shutdown) => {
            state.shutdown.replace(shutdown);
        }
        Message::DebugDraw((id, msg)) => {
            let val = state.canvases.entry((id, msg.name.clone())).or_default();
            val.id = id;
            val.name = msg.name;
            val.shapes.replace(msg.shapes);
            val.cache.clear();
            val.active = true;
        }
        Message::CloseConn(id) => {
            state.canvases.retain(|key, _| key.0 == id);
        }
        Message::Redraw(key) => {
            state.canvases.entry(key).and_modify(|it| it.cache.clear());
        }
        Message::Event(Event::Window(window::Event::Closed)) => {
            if let Some(tx) = state.shutdown.take() {
                let _ = tx.send(());
            }
        }
        Message::Event(_) => {}
        Message::CheckActive => {
            state.canvases.retain(|_, val| val.active);
            state
                .canvases
                .iter_mut()
                .for_each(|(_, val)| val.active = false);
        }
    }
}

fn view(state: &State) -> Element<Message> {
    let elm: Element<Message> = if let Some(canvas) = state.canvases.iter().next() {
        widget::canvas(canvas.1)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    } else {
        widget::text("Wait Connection.").into()
    };
    widget::center(elm).into()
}

fn subscription(_state: &State) -> Subscription<Message> {
    let net = Subscription::run(|| {
        let (tx, rx) = channel(10);

        std::thread::spawn(move || {
            let listener = Async::new(UnixListener::bind("./view_box2d.sock").unwrap()).unwrap();
            let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let shutdown = async {
                let _ = tx.clone().send(Message::IoReady(shutdown_tx)).await;
                let _ = shutdown_rx.await;
                true
            };
            let listen = async {
                let mut id = 1;
                loop {
                    if let Ok((stream, _)) = listener.read_with(|it| it.accept()).await {
                        let mut tx = tx.clone();
                        rt.spawn(async move {
                            if let Err(e) = handle_conn(id, stream, tx.clone()).await {
                                log::warn!("connection error. {e}");
                                let _ = tx.send(Message::CloseConn(id));
                            }
                        });
                    }
                    id += 1;
                }
            };
            if rt.block_on(shutdown.or(listen)) {
                drop(listener);
                let _ = std::fs::remove_file("./view_box2d.sock");
            }
        });

        rx
    });

    let events = iced::event::listen().map(Message::Event);
    let tick = iced::time::every(iced::time::Duration::from_secs(2)).map(|_| Message::CheckActive);

    Subscription::batch([net, events, tick])
}

async fn handle_conn(id: i32, stream: UnixStream, mut tx: Sender<Message>) -> anyhow::Result<()> {
    let mut frame_reader = FramedRead::new(Async::new(stream).unwrap().compat(), ShapeCodec::new());
    while let Some(result) = frame_reader.next().await {
        tx.send(Message::DebugDraw((id, result?))).await?
    }
    tx.send(Message::CloseConn(id)).await.map_err(Into::into)
}
