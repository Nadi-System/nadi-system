use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod debug {
    use crate::prelude::*;
    use abi_stable::std_types::Tuple2;
    use colored::Colorize;
    use nadi_plugin::network_func;

    /// Print the args and kwargs on this function
    ///
    /// This function will just print out the args and kwargs the
    /// function is called with. This is for debugging purposes to see
    /// if the args/kwargs are identified properly. And can also be
    /// used to see how the nadi system takes the input from the
    /// function call.
    #[network_func]
    fn debug(_net: &mut Network, #[args] args: AttrSlice, #[kwargs] kwargs: &AttrMap) {
        let mut args_str: Vec<String> = args
            .iter()
            .map(|a| Attribute::to_colored_string(a).to_string())
            .collect();
        let kwargs_str: Vec<String> = kwargs
            .iter()
            .map(|Tuple2(k, v)| format!("{}={}", k.to_string().blue(), v.to_colored_string()))
            .collect();
        args_str.extend(kwargs_str);
        println!("Function Call: debug({})", args_str.join(", "));
        println!("Args: {args:?}");
        println!("KwArgs: {kwargs:?}");
    }

    /// Echo the string to stdout or stderr
    ///
    /// This simply echoes anything given to it. This can be used in
    /// combination with nadi tasks that create files (image, text,
    /// etc). The `echo` function can be called to get the link to
    /// those files back to the stdout.
    ///
    /// Also useful for nadi preprocessor.
    #[network_func(error = false, newline = true)]
    fn echo(_net: &mut Network, line: String, error: bool, newline: bool) {
        match (error, newline) {
            (false, false) => print!("{line}"),
            (false, true) => println!("{line}"),
            (true, false) => eprint!("{line}"),
            (true, true) => eprintln!("{line}"),
        }
    }

    /// Echo the ----8<---- line for clipping sytax
    ///
    /// This function is a utility function for the generation of nadi
    /// book. This prints out the `----8<----` line when called, so
    /// that `mdbook` preprocessor for `nadi` knows where to clip the
    /// output for displaying it in the book.
    ///
    /// This makes it easier to only show the relevant parts of the
    /// output in the documentation instead of having the user see
    /// output of other unrelated parts which are necessary for
    /// generating the results.
    ///
    /// # Example
    /// Given the following tasks file:
    /// ```task
    /// net load_file("...")
    /// net load_attrs("...")
    /// net clip()
    /// net render("{_NAME} {attr1}")
    /// ```
    ///
    /// The clip function's output will let the preprocessor know that
    /// only the parts after that are relevant to the user. Hence,
    /// it'll discard outputs before that during documentation
    /// generation.
    #[network_func(error = false)]
    fn clip(_net: &mut Network, error: bool) {
        if error {
            eprintln!("----8<----");
        } else {
            println!("----8<----");
        }
    }
}
