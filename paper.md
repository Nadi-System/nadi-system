---
title: NADI -- Network Analysis and Data Integration with a Domain Specific Programming Language
tags:
  - Rust
  - hydrology
  - river
  - graph
authors:
  - name: Gaurav Atreya
    orcid: 0000-0002-0234-2165
    corresponding: true
    affiliation: 1
  - name: Todd Steissberg
    affiliation: 2
  - name: Patrick A. Ray
    affiliation: 1
affiliations:
  - name: Department of Chemical and Environmental Engineering, University of Cincinnati, OH, USA
    index: 1
  - name: U. S. Army Engineer Research and Development Center (ERDC), Davis, CA, USA
    index: 2
date: 20 May 2025
bibliography: references.bib
---

# Summary
We present the Network Analysis and Data Integration (NADI) System, a
developing software framework designed to facilitate river data
analysis. NADI System comes with a Domain Specific Programming
Language (DSL) that has an intuitive syntax for network metadata
analysis as well as a plugin system to run user-defined functions on
each node or the whole network. Plugins provide seamless integration
with other softwares and programming languages.

NADI can be used from the Command Line Interface (CLI), Graphical User
Interface (GUI) using the NADI Integrated Development Environment
(IDE), as a Rust library, or as a Python library, allowing users to
write their own plugins or programs.

# Statement of need
Hydrological analysis sometimes consists of data that is related to
points in the river, and frequently with relationships between
upstream/downstream points (e.g., higher correlation, mass
balance). Some analyses that benefit from use of such relationships
are: finding inconsistencies in the data, filling missing data, and
visualization of metadata. There is a need for intelligent
computational assistance on network based system to reduce the
workload that further improves the efficiency and reproducibility of
research in this field [@rosenbergNextFrontierMaking2020]. Specific
hydrology-focused softwares [@rossmanOpenSourcingEPANET2010;
@gironasNewApplicationsManual2010] lack general applicability, while
general purpose programming languages might not have the succinct and
intuitive syntax. This highlights the need for a balanced approach
that combines specificity to hydrological research questions with
general capabilities.

Domain Specific Programming Languages (DSLs) have several advantages
including improved code readability and maintainability due to the
tailored syntax and semantics, as well as increased efficiency through
optimized algorithms and data structures
[@mernikWhenHowDevelop2005]. Although Graphviz has made great progress
on the visualization of graph [@gansnerOpenGraphVisualization2000;
@ellsonGraphvizDynagraphStatic2004], it does not have the analytical
capabilities. Hydrolang has the capabilities of analysis and
visualization for hydrological applications, but it is web-based
[@erazoramirezHydroLangOpensourceWebbased2022]. Languages made for
grid-based spatial analysis [@pullarMapScriptMapAlgebra2001;
@kuhnDesigningLanguageSpatial2015] are not suitable for values like
streamflow which are partially spatially continuous along the river,
but not in 2D/3D space.

We present NADI System that can load a river network as a Rooted Tree
Graph [@deoGraphTheoryApplications2016] --- which is known to be one
of the best ways to represent the river network
[@rinaldoTreesNetworksHydrology2006;
@kuhnDesigningLanguageSpatial2015;
@abed-elmdoustEmergentSpectralProperties2017] --- and provide the DSL
for network metadata analysis. NADI is written in Rust due to the memory
safety [@fultonBenefitsDrawbacksAdopting2021;
@xuMemorySafetyChallengeConsidered2021;
@bugdenSafetyPerformanceProminent2022], runtime performances
[@zhangUnderstandingRuntimePerformance2023], and the macro system that
gives us the metaprogramming features to make plugin development
easier.

The figure below shows the GUI of NADI IDE, with the editor (left top), function help (left bottom), terminal (top right), network viewer, and attribute browser (right bottom). These panes can be managed in a tiling window style.

![Screenshot of the NADI IDE showcasing different components](ide-screenshot.png)

# Data Structures
Nadi has the following main data structures:

- **Node** is one point on the network. It can have input nodes, one output node, and attributes associated with it.
- **Network** consists of several connected nodes. It can also have attributes associated with it.
- **Attributes** are values that can be boolean, integer, float, string, date, time, datetime, array, and table.
- **Functions** are categorized into environment, network, and node functions based on what they work on. For example, network function is run on a network, while node function is run on each node.
- **Expressions** are a combination of attributes, variables, function calls, conditionals, etc that can result in an attribute value.
- **Propagation** is how the node functions are called in a network, you can call them in different order, based on a list, or filtered by expression.
- **Task** in NADI is an execution body consisting of type of task, optional output attribute name, and expression or function call. Only the top level function call on the expression can be a mutable call.
- **Task Context** is the runtime environment for tasks to be run. It consists of a network, environmental variables, and functions loaded from plugins.

Figure below shows how the different data types come together to generate a task and the task context. Each task runs in the task context, giving outputs, modifying the task context, or producing side effects (saving files).

![data Structures and the their relationship in the Tasks System](tasks-dtypes.png)

# Network Analysis
Once the network information in a text file, and the attributes are loaded into the NADI System, Network Analysis is done through the Task System. For example, the following code represents a task that calculates the variable y as a cumulative sum of all the values of variable x at a node and its upstream points.

```
node<inputsfirst>.y = node.x + sum(inputs.y);
```
It is equivalent to:

$$
y_{i} = x_i + \sum_{j \in I_i}{y_j}
$$

Where, $x_i, y_i$ are values of $x,y$ on node $i$ and, $I_i$ is the set of input nodes for node $i$.

@atreyaEstimatingInfluenceWater2024 demonstrates a complex task like river routing model using the network structure. For more concepts and up to date syntax, refer to the Nadi Book [@nadi-book-070].

# Extensibility
Nadi Task system supports two types of plugins for extending the use cases.

- **Compiled Plugins** are shared libraries (`.so` files in Linux, `.dll`
in Windows, and `.dynlib` in MacOS) containing a list of functions that can be loaded into the main program during runtime.
- **Executable Plugins** are independent programs that are run and
their standard output is used to communicate values back to the NADI
System.

Instructions on how to use them are available on the plugin developer guide section of Nadi Book [@nadi-book-070].

# Acknowledgements

Grant: #W912HZ-24-2-0049 Investigators:Ray, Patrick 09-30-2024 -- 09-29-2025 U.S. Army Corps of Engineers Advanced Software Tools for Network Analysis and Data Integration (NADI) 74263.03 Hold Level:Federal

# References
