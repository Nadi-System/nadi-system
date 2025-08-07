use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod logic {
    use crate::prelude::*;
    use nadi_plugin::env_func;

    /// Simple if else condition
    ///
    /// This is similar to using the `if-else` expression, the
    /// difference being the condition is relaxed. For example, for
    /// `if-else` the condition should be true or false, but for this
    /// function, the attribute can be anything that can be cast as
    /// true or false. (e.g. 1 => true, 0 => false)
    ///
    /// ```task
    /// env assert_eq(ifelse(true, 1, 2), 1)
    /// env assert_eq(ifelse(false, 1, 2), 2)
    /// env assert_eq(ifelse(100.0, 1, 2), 1)
    /// env assert_eq(ifelse(true, 1, 2), if (true) {1} else {2})
    /// ```
    ///
    /// There is a special syntax on the task system to do if-else
    /// conditions, which should be preferred over this function for
    /// easier readability.
    ///
    /// ```task
    /// if (true) {
    ///    env.somevar = 12;
    /// } else {
    ///    env.someothervar = 12;
    /// }
    /// env assert_eq(somevar, 12)
    /// env assert_eq(someothervar?, false)
    /// ```
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
    ///
    /// ```task
    /// env assert_eq(gt(1, 2), 1 > 2)
    /// env assert_eq(gt(1.0, 20), 1.0 > 20)
    /// ```
    #[env_func]
    fn gt(
        /// first attribute
        a: &Attribute,
        /// second attribute
        b: &Attribute,
    ) -> bool {
        a > b
    }

    /// Less than check
    ///
    /// ```task
    /// env assert_eq(lt(1, 2), 1 < 2)
    /// env assert_eq(lt(1.0, 20), 1.0 < 20)
    /// ```
    #[env_func]
    fn lt(
        /// first attribute
        a: &Attribute,
        /// second attribute
        b: &Attribute,
    ) -> bool {
        a < b
    }

    /// Equality than check
    ///
    /// ```task
    /// env assert_eq(eq(1, 2), 1 == 2)
    /// env assert_eq(eq(2.0, 2.0), 2.0 == 2.0)
    /// env assert_eq(eq(2.0, 2), 2.0 == 2)
    /// ```
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
    ///
    /// Similar to the operator `&` but the values are cast to boolean
    ///
    /// ```task
    /// env assert_eq(and(true, true), true)
    /// env assert_eq(and(true, false), false)
    /// env assert_eq(and(true, false), false & true)
    /// ```
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
    ///
    /// Similar to the operator `|` but the values are cast to boolean
    ///
    /// ```task
    /// env assert_eq(or(true, false), true)
    /// env assert_eq(or(false, false), false)
    /// env assert_eq(or(true, false), false | true)
    /// ```
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
    ///
    /// Similar to the operator `!` but the values are cast to boolean
    /// ```task
    /// env assert_eq(not(true), false)
    /// env assert_eq(not(false), true)
    /// env assert_eq(not(true), !true)
    /// env assert_eq(not(false), !false)
    /// ```
    #[env_func]
    fn not(
        /// attribute that can be cast to bool
        #[relaxed]
        cond: bool,
    ) -> bool {
        !cond
    }

    /// check if all of the bool are true
    ///
    /// ```task
    /// env assert_eq(all([true]), true)
    /// env assert_eq(all([false, true]), false)
    /// env assert_eq(all([true, true]), true)
    /// env assert_eq(all([false]), false)
    /// ```
    #[env_func]
    fn all(vars: &[bool]) -> bool {
        for v in vars {
            if !*v {
                return *v;
            }
        }
        true
    }

    /// check if any of the bool are true
    ///
    /// ```task
    /// env assert_eq(any([true]), true)
    /// env assert_eq(any([false, true]), true)
    /// env assert_eq(any([false, false]), false)
    /// env assert_eq(any([false]), false)
    /// ```
    #[env_func]
    fn any(vars: &[bool]) -> bool {
        for v in vars {
            if *v {
                return *v;
            }
        }
        false
    }
}
