// custom syntax highlight for the nadi related syntax
const NADI_INTERNAL_FUNCS = "all and any array attrmap clip command count count_str day debug echo eq exists first_attr float float_div float_mult float_transform get_attr gt has_attr has_outlet ifelse inputs inputs_len int isinf isna load_attrs load_file load_str load_toml_render lt max max_num min min_num month not or output parallel print_all_attrs print_attrs prod render render_nodes render_template run save_csv save_file series_csv set_attrs set_attrs_ifelse set_attrs_render set_nodesize_attrs set_series sleep sr_count sr_dtype sr_len sr_list sr_mean sr_sum sr_to_array str str_count str_find str_find_all strmap str_match str_replace subset sum table_to_markdown ts_count ts_dtype ts_len ts_list ts_print ts_print_csv type_name unique_str year";

const STRING_TEMPLATE_MODE = hljs.inherit(
    hljs.QUOTE_STRING_MODE,
    // assume strings are also templates, and highlight them
    // that way actually they are used as template or not
    // depends on the function (TODO maybe use different data
    // type for templates something like: `t""`)
    {
	subLanguage: "string-template"
    },
);

const NODE_LIST_OR_PATH = {
    // This is for the nodes list or path for the node functions
    scope: "meta",
    begin: '\\[',
    end: '\\]',
    className:"string",
    excludeBegin: true,
    excludeEnd: true,
    contains: [
	hljs.HASH_COMMENT_MODE,
	hljs.QUOTE_STRING_MODE,
	{
	    scope: "meta",
	    begin: '->',
	    className:"built_in",
	},
	{
	    scope: "meta",
	    begin: ',',
	    className:"built_in",
	},
    ]
};

hljs.registerLanguage("task", (hljs) => ({
    name: "Task",
    keywords: {
	keyword: "help",
	built_in: NADI_INTERNAL_FUNCS,
	literal: "false true",
    },
    contains: [
	STRING_TEMPLATE_MODE,
	hljs.HASH_COMMENT_MODE,
	hljs.C_NUMBER_MODE,
	NODE_LIST_OR_PATH,
	{
	    begin: '\\b(node|network|env|exit|end|help|inputs|output|nodes|if|else|while|in|match|function|map|attrs|loop|for|inf|nan)\\b',
	    className: "deletion",
	},
	{
	    begin: '<(sequential|inverse|inputsfirst|outputfirst|seq|inv|inp|out)>',
	    className: "regexp",
	},
    ],
}));

hljs.registerLanguage("output", (hljs) => ({
    name: "Output",
    keywords: {
	literal: "nadi:var:",
    },
    contains: [
	hljs.QUOTE_STRING_MODE,
	hljs.C_NUMBER_MODE,
	NODE_LIST_OR_PATH,
	{
	    begin: '^[$] ',
	    end: '$',
	    className: "addition",
	},
    ],
}));

hljs.registerLanguage("error", (hljs) => ({
    name: "Error",
    contains: [
	{
	    begin: '^',
	    end: '$',
	    className: "deletion",
	}
    ]
}));

// for function signatures saved in plugin help system
hljs.registerLanguage("signature", (hljs) => ({
    name: "Signature",
    aliases: ["sig"],
    keywords: {
	keyword: "Bool String Integer Float Date Time DateTime Array Table PathBuf Template Attribute AttrMap Option str bool",
	built_in: NADI_INTERNAL_FUNCS,
	literal: "false true",
    },
    contains: [
	STRING_TEMPLATE_MODE,
	{
	    begin: '^(node|network|env)',
	    className: "deletion",
	}
    ]
}));


// network connections comments and node -> node syntax
hljs.registerLanguage("network", (hljs) => ({
    name: "Network",
    aliases: [ 'net' ],
    contains: [
	hljs.QUOTE_STRING_MODE,
	hljs.HASH_COMMENT_MODE,
	{
	    scope: "meta",
	    begin: '->',
	    className:"built_in",
	},
    ]
}));

// syntax for the table template format
hljs.registerLanguage("table", (hljs) => ({
    name: "Table",
    aliases: [ 'tab' ],
    contains: [
	hljs.QUOTE_STRING_MODE,
	hljs.HASH_COMMENT_MODE,
	{
	    scope: "meta",
	    begin: /^/,
	    end: /$/,
	    className:"title",
	    contains: [
		{
		    begin: '[<>^]',
		    className: "built_in",
		},
		{
		    begin: '=>',
		    end: '$',
		    excludeBegin: true,
		    className:"name",
		    subLanguage: "string-template"
		},

	    ]
	},
    ]
}));

// syntax highlight for the string template system

// this breaks the syntax, not used right now
const DATE_TIME_FMT = {
    contains: [
	{
	    begin:'[%][a-zA-Z]',
	    className: "string"
	}
    ]
};
const TRANSFORMER = {
    begin: ':[a-zA-Z_]+(.*?)',
    keywords: "f case calc count repl take trim comma group q",
    // if the function is not a known transformer for string-template
    className:"deletion",
    contains: [
	hljs.QUOTE_STRING_MODE,
	{
	    begin: '\\(',
	    end: '\\)',
	    excludeBegin: true,
	    excludeEnd: true,
	    className: "literal",
	}
    ]
}
const LISP_LIST = {
    begin: '\\(\\s*',
    end: '\\)',
    keywords: {
	// functions from rust_lisp + stp are keyword
	keyword: "print is_null is_number is_symbol is_boolean is_procedure is_pair car cdr cons list nth sort reverse map filter length range hash hash_get hash_set st+num st+has st+var",
	// functions in std lisp/elisp are built_in
	built_in: "interactive format defun define set defmacro lambda quote list cons loop let message begin cond if and or apply eval not",
	literal: "t nil",
    },
};

LISP_LIST.contains = [
    LISP_LIST,
    hljs.QUOTE_STRING_MODE,
    hljs.C_NUMBER_MODE,
    hljs.COMMENT(';', '$'),
    {
	begin: "'",
	className: 'regexp',
	contains: [LISP_LIST],
    },
]


// since current hightlight.js used by mdbook doesn't have lisp, and
// the one in github doesn't look that good anyway
hljs.registerLanguage("elisp", (hljs) => ({
    name: "emacs-lisp",
    contains: [
	LISP_LIST
    ],
}));

const LISP_EXPR = {
    scope: "meta",
    begin: '=\\(',
    end: '\\)',
    keywords: {
	// these are not working right now, idk why?
	keyword: "print is_null is_number is_symbol is_boolean is_procedure is_pair car cdr cons list nth sort reverse map filter length range hash hash_get hash_set truncate not apply eval",
	built_in: "st+num st+has st+var",
	literal: "nil t",
    },
    className: 'title',
    contains: [LISP_LIST]
};

const TEMPLATE_PART = {
    variants: [
	hljs.QUOTE_STRING_MODE,
	TRANSFORMER,
	LISP_EXPR,
	// DATE_TIME_FMT,
    ]
}
const TEMPLATE = {
    scope: "meta",
    begin: '{',
    end: '}',
    className:"regexp",
    contains: [
	TEMPLATE_PART,
	{
	    begin: '[?]',
	    className:"built_in",
	}
    ]
};
const CMD = {
    scope: "meta",
    begin: '[$]\\(',
    end: '\\)',
    className: 'deletion',
    contains: [
	hljs.QUOTE_STRING_MODE,
	{
	    begin: '--.*?\\s',
	    className: 'keyword',
	},
	{
	    begin: '-[a-zA-Z]\\s',
	    className: 'keyword',
	},
	TEMPLATE,
    ]
};

hljs.registerLanguage("string-template", (hljs) => ({
    name: "String Template",
    aliases: [ 'stp' ],
    contains: [
	{
	    variants: [
		TEMPLATE,
		LISP_EXPR,
		CMD,
	    ],
	},
	// this will make it so that the escaped braces don't get
	// caught with template
	{
	    begin: '\\\\{',
	    className: "addition",
	},
	{
	    begin: '\\\\}',
	    className: "addition",
	},
	{
	    begin: '\\\\"',
	    className: "addition",
	},
	{
	    begin: '\\\\\\\\',
	    className: "addition",
	}
    ]
}));


hljs.registerLanguage("string-template-markdown", (hljs) => ({
    name: "String Template Markdown",
    aliases: [ 'stp-md' ],
    contains: [
	{
	    begin: '^<!-- .* -->$',
	    className: 'comment',
	    contains: [
		{
		    begin: '---8<---:[[]',
		    end: '[]]',
		    contains: [ NODE_LIST_OR_PATH]
		}
	    ]
	},
	{
	    begin: '^',
	    end: '$',
	    subLanguage: 'string-template'
	},
    ],
}));

hljs.initHighlightingOnLoad();
