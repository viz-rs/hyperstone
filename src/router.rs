use crate::Method;

#[derive(Debug)]
pub struct Router<T> {
    inherit: bool,
    path: String,
    name: Option<String>,
    tree: path_tree::PathTree<T>,
    routes: Option<Vec<(String, usize, T)>>,
}

impl<T: Clone> Router<T> {
    pub fn new() -> Self {
        Self {
            inherit: false,
            path: "/".to_string(),
            name: None,
            tree: path_tree::PathTree::new(),
            routes: None,
        }
    }

    pub fn path(mut self, path: &str) -> Self {
        self.path = join_paths(path, "");
        self
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name.replace(name.to_owned());
        self
    }

    pub fn inherit(mut self, b: bool) -> Self {
        self.inherit = b;
        self
    }

    pub fn with(mut self) -> Self {
        self
    }

    fn on(mut self, method: Method, path: impl AsRef<str>, handler: T) -> Self {
        let m = method.as_str();
        let i = m.len();
        self.routes.get_or_insert_with(Vec::new).push((
            m.to_owned() + &join_paths(&self.path, path.as_ref()),
            i,
            handler,
        ));
        self
    }

    pub fn options(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::OPTIONS, path, handler)
    }

    pub fn get(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::GET, path, handler)
    }

    pub fn post(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::POST, path, handler)
    }

    pub fn put(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::PUT, path, handler)
    }

    pub fn delete(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::DELETE, path, handler)
    }

    pub fn head(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::HEAD, path, handler)
    }

    pub fn trace(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::TRACE, path, handler)
    }

    pub fn connect(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::CONNECT, path, handler)
    }

    pub fn patch(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::PATCH, path, handler)
    }

    pub fn any(self, path: impl AsRef<str>, handler: T) -> Self {
        self.on(Method::from_bytes(&[b'*']).unwrap(), path, handler)
    }

    pub fn scope(mut self, mut router: Self) -> Self {
        if let Some(routes) = router.routes.take() {
            let r = &routes
                .iter()
                .cloned()
                .map(|mut t| {
                    t.0 = t.0[..t.1].to_owned() + &join_paths(&self.path, &t.0[t.1..]);
                    t
                })
                .collect::<Vec<_>>();
            self.routes
                .get_or_insert_with(Vec::new)
                .extend_from_slice(r);
        }
        self
    }

    pub fn serve_static(mut self, path: impl AsRef<str>) -> Self {
        self
    }
}

fn join_paths(a: &str, b: &str) -> String {
    if b.is_empty() {
        return a.to_owned();
    }
    a.trim_end_matches('/').to_owned() + "/" + b.trim_start_matches('/')
}

#[cfg(test)]
mod tests {
    use super::Router;

    #[test]
    fn routing() {
        let app = Router::<usize>::new();

        let api = Router::new().path("/api").get("/", 1);

        let v1 = Router::new().path("/v1").get("/", 2);

        let v2 = Router::new().path("/v2").get("/", 3);

        let app = app
            .scope(api.scope(v1).scope(v2))
            .get("/about", 1)
            .post("/login", 2)
            .delete("/logout", 3)
            .any("/*", 4);

        dbg!(app);
    }
}
