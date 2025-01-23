use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod logic {
    use crate::prelude::*;
    use nadi_plugin::env_func;

    /// Simple if else condition
    #[env_func]
    fn ifelse(
        /// Attribute that can be cast to bool value
        #[relaxed]
        cond: bool,
        /// Output if `cond` is true
        iftrue: Attribute,
        /// Output if `cond` is false
        iffalse: Attribute,
    ) -> Result<Attribute, String> {
        let v = if cond { iftrue } else { iffalse };
        Ok(v)
    }

    /// Greater than check
    #[env_func]
    fn gt(
        /// first attribute
        a: &Attribute,
        /// second attribute
        b: &Attribute,
    ) -> bool {
        a > b
    }

    /// Greater than check
    #[env_func]
    fn lt(
        /// first attribute
        a: &Attribute,
        /// second attribute
        b: &Attribute,
    ) -> bool {
        a < b
    }

    /// Greater than check
    #[env_func]
    fn eq(
        /// first attribute
        a: &Attribute,
        /// second attribute
        b: &Attribute,
    ) -> bool {
        a == b
    }

    /// Boolean and
    #[env_func]
    fn and(
        /// List of attributes that can be cast to bool
        #[args]
        conds: &[Attribute],
    ) -> bool {
        let mut ans = true;
        for c in conds {
            ans = ans && bool::from_attr_relaxed(c).unwrap();
        }
        ans
    }

    /// boolean or
    #[env_func]
    fn or(
        /// List of attributes that can be cast to bool
        #[args]
        conds: &[Attribute],
    ) -> bool {
        let mut ans = false;
        for c in conds {
            ans = ans || bool::from_attr_relaxed(c).unwrap();
        }
        ans
    }

    /// boolean not
    #[env_func]
    fn not(
        /// attribute that can be cast to bool
        #[relaxed]
        cond: bool,
    ) -> bool {
        !cond
    }
}
