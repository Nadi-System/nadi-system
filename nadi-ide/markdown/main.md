# Network Analysis and Data Integration (NADI)
NADI is group of software packages that facilitate network analysis and do data analysis on data related to network/nodes.


This page contains a brief explanation of core concepts. Refer to the
online NADI book for full explanations with examples, as well as other
sections like developers guide and example usages.

The online nadi book is here: [https://nadi-system.github.io/](https://nadi-system.github.io/)

Click at the book icon on the top bar to visit the online book. Click
on the github icon to visit the source codes for nadi system.

## Node

A Node is a point in network. A Node can have multiple input nodes and
only one output node. And a Node can also have multiple attributes
identifiable with their unique name, along with timeseries values also
identifiable with their names.

If you understand graph theory, then node in nadi network is the same
as a node in a graph.

## Network

A Network is a collection of nodes. The network can also have
attributes associated with it. The connection information is stored
within the nodes itself. But Network will have nodes ordered based on
their connection information. So that when you loop from node from
first to last, you will always find output node before its input
nodes.

A condition a nadi network is that it can only be a directed graph
with tree structure.

Example Network file:
```net
# network consists of edges where input node goes to output node
# each line is of the format: input -> output
tenessee -> ohio
# if your node name has characters outside of a-zA-Z_, you need to
# quote them as strings
ohio -> "lower-mississippi"
"upper-mississippi" -> "lower-mississippi"
missouri -> "lower-mississippi"
arkansas -> "lower-mississippi"
red -> "lower-mississippi"
```

## Attributes
Attributes are TOML like values. They can be one of the following types:

| Type Name | Rust Type         | Description                             |
|-----------|-------------------|-----------------------------------------|
| Bool      | `bool`            | Boolean values (`true` or `false`)      |
| String    | `RString`         | Quoted String Values                    |
| Integer   | `i64`             | Integer values (numbers)                |
| Float     | `f64`             | Float values (numbers with decimals)    |
| Date      | `Date`            | Date (`yyyy-mm-dd` formatted)           |
| Time      | `Time`            | Time (`HH:MM`, `HH:MM:SS` formatted)    |
| DateTime  | `DateTime`        | Date and Time separed by ` ` or `T`     |
| Array     | `RVec<Attribute>` | List of any attribute values            |
| Table     | `AttrMap`         | Key Value pairs of any attribute values |


Example Attribute File:
```toml
river = "Ohio River"
outlet = "Smithland Lock and Dam"
outlet_is_gage = true
outlet_site_no = ""
streamflow_start = 1930-06-07
mean_streamflow = 123456.0
obs_7q10 = 19405.3
nat_7q10 = 12335.9
num_dams_gages = 2348
```

## String Template
String templates are strings with dynamic components that can be
rendered for each node based on the node attributes.

## Node Function
Node function runs on each node. It takes arguments and keyword arguments.

For example following node function takes multiple attribute names and prints them. The signature of the node function is `print_attrs(*args)`.

```task
network load_file("mississippi.net")
node print_attrs("NAME", "INDEX")
```

Only the `NAME` is printed as they do not have any other attributes.

You can customize execution of node functions, like you can
selectively run only a few nodes, or change the order the nodes are
executed.

- Reverse the execution order,
- Run it only on a list of nodes,
- Run it on a path (from one node to another), and
- Run it on the nodes satisfying a condition.

## Network Function
Network function runs on the network as a whole. It takes arguments and keyword arguments.

## Task
Tasks system acts like a pseudo scripting language for nadi system. A
Task is a function call that can either be a node function or a
network function. Functions are unique based on their names, and can
have.

The code examples throughout this page are scripts using the task system.

Here is an example contents of a task file:
```task
node print_attrs("uniqueID")
node show_node()
network save_graphviz("test.gv")
# path for node selection
node[WV04112 -> WV04113] render("=(> 2 3)")
```

## Further Reading
Now that you have the overview of the nadi system's data
structures. Please refer to the individual function's help to see how
they are used and what they do.
