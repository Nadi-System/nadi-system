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
  - name: Department of Chemical and Environmental Engineering, University of Cincinnati, 601, Engineering Research Center, Cincinnati, OH 45221-0012, United States of America
    index: 1
  - name: U. S. Army Engineer Research and Development Center (ERDC), 707 Fourth St., Davis, CA 95616, United States of America
    index: 2
date: 20 May 2025
bibliography: references.bib
---

# Summary
We present Network Analysis and Data Integration (NADI) System, a
developing software framework designed to facilitate river data
analysis and visualization. NADI comes with a Domain Specific
Programming Language (DSL) that has an intuitive syntax for network
metadata analysis as well as a generalized plugin system to run
user-defined functions on each node or the whole network. Plugins
provide seamless integration with other softwares and
programming languages.

# Statement of need
Hydrologic modeling, which involves integrating diverse data to
simulate complex processes [@singhMathematicalModelingWatershed2002;
@clarkImprovingRepresentationHydrologic2015;
@loucksWaterResourceSystems2017], is hindered by manual and
time-consuming data organization tasks, resulting on time drain,
disengagement, information overload and human errors
[@brunsTediousWorkDeveloping2024, @readStateScienceEvolving2021].

A directed acyclic graph (DAG) also called directed tree
[@deoGraphTheoryApplications2016] is one of the best way to represent
the river network [@rinaldoTreesNetworksHydrology2006;
@kuhnDesigningLanguageSpatial2015;
@abed-elmdoustEmergentSpectralProperties2017], characterized by
directed edges and a hierarchical structure, which offers efficient
storage and retrieval of large databases compared to relational models
[@demirOptimizationRiverNetwork2017; @knoxOpensourceDataManager2019].

Domain Specific Programming Language (DSL) has several advantages,
including improved code readability and maintainability due to the
DSL's tailored syntax and semantics, increased efficiency through
optimized algorithms and data structures, and enhanced collaboration
among domain experts and programmers by providing a shared language
and problem-solving framework [@mernikWhenHowDevelop2005]. Although
Graphviz has made great progress on the visualization of graph
[@gansnerOpenGraphVisualization2000;
@ellsonGraphvizDynagraphStatic2004], it does not have the analytical
capabilities. Hydrolang has the capabilities of analysis and
visualization for hydrological applications, but it is web based
[@erazoramirezHydroLangOpensourceWebbased2022]. Languages made for
spatial analysis are either working with grid based system
[@pullarMapScriptMapAlgebra2001; @kuhnDesigningLanguageSpatial2015].

Rust was chosen as implementation language due to its features like
like memory safety [@fultonBenefitsDrawbacksAdopting2021;
@xuMemorySafetyChallengeConsidered2021;
@bugdenSafetyPerformanceProminent2022] --- which is a recommended
practice for new programs [@FinalONCDTechnicalReport], its runtime
performaces [@zhangUnderstandingRuntimePerformance2023], and its macro
system which gives us the metaprogramming features to make plugin
development easier.


# Software Components
NADI can be used from Command Line Interface (CLI), Graphical User Interface (GUI) using the NADI Integrated Development Environment (IDE), as a Rust library, or as a Python library.

The figure below shows the GUI of NADI IDE, with the editor (left top), function help (left bottom), terminal (top right), network viewer, and attribute browser (right bottom). These panes can be managed a tiling window style.

![Screenshot of the NADI IDE showcasing different components](ide-screenshot.png)

# Data Structures
Nadi has the following main data structures:

- **Node** is one point on the network. It can have input nodes, one output node, and attributes associated with it.
- **Network** consists of several interconnected nodes. It has to be DAG with only one outlet node. It can have attributes associated with it.
- **Attributes** are values that can be boolean, integer, float, string, array, table, etc.
- **Functions** are categorized into environment, network and node functions based on what they work on. For example, network function is run on a network, while node function is run at each node.
- **Expressions** are combination of attributes, variables, function calls, if--else, etc that can result in an attribute value.
- **Propagation** is how the node functions are called in a network, you can call them in different order, based on a list, or filtered by expression.
- **Task** in NADI is an execution body consisting of type of the task, optional output attribute name, and an expression that may or may not return a value. Only the top level function call on the expression can be a mutable call.
- **String Templates** are strings with variables and transformation functions inside them that can be used to render it into different strings dynamically based on network/node attributes.
- **Task Context** is the runtime environment for tasks to be run. It consists of a network, environmental variables and functions loaded from plugins.

Figure below shows how the different data types come together to generate a task and the task context. Each task runs in the task context, giving outputs, modifying the context, producing side effects (saving files), etc.

![data Structures and the their relationship in the Tasks System](tasks-dtypes.png)

# Key Features

## Network Analysis
Once the network information in a text file, and the attributes are loaded into the NADI System, Network Analysis is done through the Task System. For example, the following code represents a task that calculates the variable y as a cumulative sum of all the values of variable x at a node and its upstream points.

```
node<inputsfirst>.y = node.x + sum(inputs.y);
```
It is equivalent to:

$$
y_{i} = x_i + \sum_{j=0 \forall j \in I_i}^{n}{y_j}
$$

Where, \(x_i, y_i\) are values of \(x,y\) on node \(i\) and, \(I_i\) is the set of input nodes for node \(i\).

@atreyaEstimatingInfluenceWater2024 demonstrates a complex task like river routing model using the network structure. For more examples of the up to date codes refer to the [Nadi Book](https://nadi-system.github.io/).

## Extensibility
Nadi Task system supports two types of plugins for extending the use case to suit the need of its users.

- **Compiled Plugins** are shared libraries (`.so` files in Linux, `.dll`
in Windows, and `.dynlib` in MacOS) containing a list of functions that can be loaded into the main program during runtime.
- **Executable Plugins** are independent programs/commands that are
run and their standard output is used to communicate values back to
the NADI system.

Furthermore, NADI can be used as a library in both Rust and Python, allowing users to write their own plugins or programs.

# Acknowledgements

Grant: #W912HZ-24-2-0049 Investigators:Ray, Patrick 09-30-2024 -- 09-29-2025 U.S. Army Corps of Engineers Advanced Software Tools for Network Analysis and Data Integration (NADI) 74263.03 Hold Level:Federal

# References
