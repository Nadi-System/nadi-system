/// This is the signature of the functions that plugins should
/// provide, for dealing with node or network
///
/// An example plugin is given for rust and C.
pub type Function<T> = extern "C" fn(*mut T, *mut FunctionCtx);

pub struct FunctionCtx {
    args: Vec<toml::Value>,
    kwargs: toml::Table,
    result: anyhow::Result<()>,
}

impl Default for FunctionCtx {
    fn default() -> Self {
        Self {
            args: vec![],
            kwargs: toml::Table::new(),
            result: Ok(()),
        }
    }
}

impl FunctionCtx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_args(mut self, args: Vec<toml::Value>) -> Self {
        self.args = args.clone();
        args.into_iter()
            .filter_map(|a| a.as_table().cloned())
            .for_each(|t| {
                self.kwargs.extend(t);
            });
        self
    }

    pub fn arg(&self, ind: usize) -> Option<&toml::Value> {
        self.args.get(ind)
    }

    pub fn kwarg(&self, name: &str) -> Option<&toml::Value> {
        self.kwargs.get(name)
    }

    pub fn args_count(&self) -> usize {
        self.args.len()
    }

    pub fn set_error(&mut self, err: anyhow::Error) {
        self.result = Err(err);
    }

    pub fn error(&self) -> Option<String> {
        self.result.as_ref().err().map(|e| e.to_string())
    }
}
