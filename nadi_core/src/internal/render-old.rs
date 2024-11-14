use super::NodeFunction_TO;
use crate::functions::{NadiFunctions, NetworkFunction, NodeFunction};
use crate::plugins::NadiPlugin;
use crate::{AttrMap, AttrSlice, Attribute, NodeInner};
use abi_stable::sabi_trait::{TD_CanDowncast, TD_Opaque};
use abi_stable::std_types::{
    ROption::{self, RNone, RSome},
    RString, RVec,
};
use string_template_plus::Template;

pub struct RenderMod;

impl NadiPlugin for RenderMod {
    fn name(&self) -> RString {
        "render".into()
    }
    fn register(&self, nf: &mut NadiFunctions) {
        nf.register_node_function(NodeFunction_TO::from_value(RenderText, TD_CanDowncast));
    }
}

#[derive(Debug)]
pub struct RenderText;

impl NodeFunction for RenderText {
    fn name(&self) -> RString {
        "rendertext".into()
    }

    fn help(&self) -> RString {
        "Render the given template for the node\n".into()
    }

    fn call(&self, node: &mut NodeInner, args: AttrSlice, _kwargs: &AttrMap) -> ROption<Attribute> {
        let arg0 = args.first().expect("Text template not given");
        let cmd_templ: String = crate::attrs::FromAttribute::from_attr(arg0.clone())
            .expect("Text template needs to be string");
        let templ = Template::parse_template(&cmd_templ).unwrap();
        let text = node.render(templ).unwrap();
        println!("{text}");
        RSome(Attribute::String(text.into()))
    }
}
