use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct Signal<T> {
    value: Rc<RefCell<T>>,
}

impl<T> Signal<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Rc::new(RefCell::new(value)),
        }
    }

    pub fn set(&self, next: T) {
        *self.value.borrow_mut() = next;
    }

    pub fn update(&self, update: impl FnOnce(&mut T)) {
        let mut value = self.value.borrow_mut();
        update(&mut value);
    }
}

impl<T: Clone> Signal<T> {
    pub fn get(&self) -> T {
        self.value.borrow().clone()
    }
}

pub fn signal<T>(value: T) -> Signal<T> {
    Signal::new(value)
}

#[derive(Clone)]
pub struct Mem<T> {
    compute: Rc<dyn Fn() -> T>,
}

impl<T> Mem<T> {
    pub fn new(compute: impl Fn() -> T + 'static) -> Self {
        Self {
            compute: Rc::new(compute),
        }
    }

    pub fn get(&self) -> T {
        (self.compute)()
    }
}

pub fn mem<T>(compute: impl Fn() -> T + 'static) -> Mem<T> {
    Mem::new(compute)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EffectHandle;

pub fn effect(run: impl FnMut() + 'static) -> EffectHandle {
    let mut run = run;
    run();
    EffectHandle
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceState<T, E> {
    Loading,
    Ready(T),
    Error(E),
}

#[derive(Debug, Clone)]
pub struct Resource<T, E> {
    state: Rc<RefCell<ResourceState<T, E>>>,
}

impl<T, E> Resource<T, E> {
    pub fn loading() -> Self {
        Self {
            state: Rc::new(RefCell::new(ResourceState::Loading)),
        }
    }

    pub fn from_result(result: Result<T, E>) -> Self {
        let state = match result {
            Ok(value) => ResourceState::Ready(value),
            Err(error) => ResourceState::Error(error),
        };

        Self {
            state: Rc::new(RefCell::new(state)),
        }
    }

    pub fn state(&self) -> ResourceState<T, E>
    where
        T: Clone,
        E: Clone,
    {
        self.state.borrow().clone()
    }
}

pub fn resource<T, E, F, Fut>(_loader: F) -> Resource<T, E>
where
    F: FnOnce() -> Fut + 'static,
    Fut: Future<Output = Result<T, E>> + 'static,
{
    // Draft API only: the async scheduler/runtime will own actual execution later.
    Resource::loading()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxNode {
    Element {
        tag: &'static str,
        children: Vec<AxNode>,
    },
    Text(String),
}

pub fn text(content: impl Into<String>) -> AxNode {
    AxNode::Text(content.into())
}

pub fn element(tag: &'static str, children: Vec<AxNode>) -> AxNode {
    AxNode::Element { tag, children }
}

pub fn view(build: impl FnOnce() -> AxNode) -> AxNode {
    build()
}

pub mod prelude {
    pub use super::element;
    pub use super::effect;
    pub use super::mem;
    pub use super::resource;
    pub use super::signal;
    pub use super::text;
    pub use super::view;
    pub use super::AxNode;
    pub use super::EffectHandle;
    pub use super::Mem;
    pub use super::Resource;
    pub use super::ResourceState;
    pub use super::Signal;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_and_mem_work_together() {
        let count = signal(2);
        let count_for_mem = count.clone();
        let doubled = mem(move || count_for_mem.get() * 2);

        assert_eq!(doubled.get(), 4);
        count.set(5);
        assert_eq!(doubled.get(), 10);
    }

    #[test]
    fn effect_runs_immediately_in_draft_runtime() {
        let hit_count = signal(0);
        let hit_count_for_effect = hit_count.clone();

        let _handle = effect(move || {
            hit_count_for_effect.update(|value| *value += 1);
        });

        assert_eq!(hit_count.get(), 1);
    }

    #[test]
    fn resource_from_result_tracks_ready_state() {
        let posts = Resource::<u32, &'static str>::from_result(Ok(3));
        assert_eq!(posts.state(), ResourceState::Ready(3));
    }
}
