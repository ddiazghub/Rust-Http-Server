use std::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::error::Error;
use std::marker::PhantomData;
use std::str::Split;
use crate::error::{DefaultError, ServerError};
use crate::message::{Request, Response};
use crate::method::HttpMethod;

pub static NOT_FOUND_ACTION: fn(&Request) -> Result<Response, DefaultError> = |_| Err(DefaultError::NotFound);

pub trait RouteAction<E: ServerError> : Fn(&Request) -> Result<Response, E> + Sync + Send + Clone + 'static {}
impl <E: ServerError, F: Fn(&Request) -> Result<Response, E> + Sync + Send + Clone + 'static> RouteAction<E> for F {}

pub struct Router<E: ServerError, F: RouteAction<E>> {
    nothing: PhantomData<E>,
    route_tree: [RoutingTreeNode<E, F>; 5],
    not_found_action: F
}

impl <E: ServerError, F: RouteAction<E>> Router<E, F> {
    pub fn new(not_found_action: F) -> Self {
        Self {
            nothing: PhantomData,
            route_tree: [
                RoutingTreeNode::new(not_found_action.clone()),
                RoutingTreeNode::new(not_found_action.clone()),
                RoutingTreeNode::new(not_found_action.clone()),
                RoutingTreeNode::new(not_found_action.clone()),
                RoutingTreeNode::new(not_found_action.clone())
            ],
            not_found_action
        }
    }

    pub fn get(&self, method: HttpMethod, route: &str) -> &F {
        println!("{route}");
        let path = Self::split_route(route);
        println!("{:?}", path.clone().collect::<Vec<&str>>());

        match self.route_tree[method as usize].get(path) {
            Some(action) => action,
            None => &self.not_found_action
        }
    }

    pub fn add(&mut self, method: HttpMethod, route: &str, action: F) {
        let path = Self::split_route(route);
        self.route_tree[method as usize].add(path, action, &self.not_found_action);
    }

    fn split_route(route: &str) -> Split<char> {
        route.trim_matches('/').split('/')
    }
}

pub struct RoutingTreeNode<E: ServerError, F: RouteAction<E>> {
    nothing: PhantomData<E>,
    action: F,
    children: HashMap<String, Box<RoutingTreeNode<E, F>>>
}

impl <E: ServerError, F: RouteAction<E>> RoutingTreeNode<E, F> {
    pub fn new(action: F) -> Self {
        Self {
            nothing: PhantomData,
            action,
            children: HashMap::new()
        }
    }

    pub fn get<'a, I: Iterator<Item = &'a str>>(&self, mut route: I) -> Option<&F> {
        let p = route.next();
        println!("{p:?}");

        match p {
            Some("") | None => Some(&self.action),
            Some(next) => match self.children.get(next) {
                Some(child) => child.get(route),
                _ => None
            },
        }
    }

    pub fn add<'a, I: Iterator<Item = &'a str>>(&mut self, mut route: I, action: F, not_found_action: &F) {
        let p = route.next();
        println!("{p:?}");

        match p {
            Some("") | None => self.action = action,
            Some(next) => {
                if !self.children.contains_key(next) {
                    self.children.insert(next.to_string(), Box::new(RoutingTreeNode::new(not_found_action.clone())));
                }

                let child = self.children.get_mut(next).unwrap();
                child.add(route, action, not_found_action);
            }
        }
    }
}