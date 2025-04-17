---
title: "NADI: A Free and Open Source Generalized Modeling Platform for Data Integration using River Network Information"
tags:
  - Rust
  - hydrology
  - river network
  - graph
  - environmental modeling
authors:
  - name: Gaurav Atreya
    orcid: 0000-0002-0234-2165
	corresponding: true
	affiliation: 1
  - name: Patrick A. Ray
    orcid: 0000-0001-9495-2317
	affiliation: 1
  - name: Todd Steissberg
    affiliation: 2
affiliations:
  - name: Department of Chemical and Environmental Engineering, University of Cincinnati, Cincinnati, OH, United States of America
    index: 1
  - name: U. S. Army Engineer Research and Development Center (ERDC), Davis, CA, United States of America
    index: 2
date: 20 April 2025
bibliography: references.bib
---

# Summary
NADI is a developing software framework consisting of rust library,
python library, Command Line Interface (CLI), and Graphical User
Interface (GUI). NADI provides a combination of features that
complement existing tools, including spatial and temporal
interpolation algorithms based on network connectivity, interactive
plotting capabilities, integration with different modeling tools,
smart model run with a dependency system, batch report generation,
error calculations, and a generalized plugin system to run
user-defined functions on each node or the whole network. Our approach
enables users to spend less time on data preparation and visualization
and more time developing actual model algorithms, ultimately enhancing
the accuracy and comprehensiveness of hydrological models. This paper
explains the design principles and the functions of the NADI system.

# Statement of Need
Hydrologic modeling involves the integration of diverse data to
simulate complex (and often poorly understood) hydrological processes
[@singhMathematicalModelingWatershed2002a;
@clarkImprovingRepresentationHydrologic2015a;
@loucksWaterResourceSystems2017]. The development and calibration of
hydrologic models involves time-intensive, repetitive data
organization tasks during the initial phases of data pre-processing
and visualization. Manually (or semi-manually) organizing and
error-correcting large datasets to ensure consistency and accuracy is
a common time drain faced by researchers, which may result in
disengagement and information overload, and data processing mistakes
[@brunsTediousWorkDeveloping2024]. Considering the ever-increasing
research work being done in the field of water resources systems
planning and management [@bornmannGrowthRatesModern2021], the need for
intelligent computational assistance is high.

To compensate for these challenges, researchers may remove rows of
missing data (i.e., the deletion method), employ statistical models
that approximate missing values, or use imputation methods such as
nearest neighbor, linear interpolation, or statistical inference, to
fill data gaps [@hockeGapFillingNoise2009;
@hamzahImputationMethodsRecovering2020;
@hamzahComparisonMultipleImputation2021].

The widespread adoption of Geographic Information System (GIS)
software across academia, government agencies, and the private sector
has enabled more confident decision-making based on spatial data, and
driven research innovation. The use of GIS system has also been found
useful in hydrological modeling [@martinInterfacingGISWater2005]. The
ubiquitousness and uniformity of GIS software has partially addressed
the complexities of managing and analyzing diverse datasets in various
formats [@devantierReviewGisApplications1993;
@tsihrintzisUseGeographicInformation1996]. By integrating spatial and
non-spatial data, GIS platforms facilitate efficient collection,
archival, analytical, and visualization processes, thereby enabling
stakeholders to better understand and address pressing issues in their
respective domains.

# Design Principles
The river network on which NADI is based is a directed acyclic graph
(DAG) whose underlying undirected graph is a tree. The features of
such graphs are: directed edges (i.e., edges have a start and an end
that cannot be interchangeable), weakly connected nodes (i.e., there
is a path from any node to any other node when directed edges are
replaced with undirected edges), planar dimensionality (i.e., network
configurations can be drawn in a plane so that no two edges are
intersected) [@deoGraphTheoryApplications2016].

The main components of Nadi System are follows:
- Nadi Geographic Information System (GIS) Tool
- `nadi_core` library (rust)
- Python Library (`nadi-py`)
- Nadi Command Line Interface (CLI)
- The Nadi Integrated Development Environment (IDE)
- Nadi Plugins

![Scrrenshot of the Nadi IDE Showing the (1) Task Editor, (2) Terminal (3) Network Viewer, and (4) Node Attribute Viewer](./nadi-ide/images/ide.png)

For more details on the data structure, implementation details, and demostration of capabilities please refer to the individual documentations, or to the Nadi Book (User Manual).

# Features
## Network Detection
Here we load a stream network, and points of interest. Then we snap
those points to the network using R-tree data structure to find
nearest points in streams [@guttmanRtreesDynamicIndex1984]. We then
find the connections between the points and output the network in a
text format. The tool also has capabilities to download the stream
network from NHDPlus, USGS streamgages, basin boundary and such given
a USGS stream gage site ID.

In case of simple networks, you can skip this process and simply write
your network information in a text file and proceed.

## Network Visualization
Nadi has the ability to output a tabular form of metadata with network visualization on the side. It also has the capabilities to generate interactive PDFs and HTMLs using the network information as index and 

## Network Analysis
Plug-ins to the `nadi` core structure provide its analytical
capabilities. Data analytic and visualization capabilities benefit
from the `nadi` network information, which input/export into various
standard formats (e.g., GIS). The generalized plugin system
facilitates creation of custom functions, which operate either on the
whole network simultaenously, or selected nodes individually.

## Extensibility
The extensibility of the NADI system comes from the plugin system --
which is implemented in two ways: compiled plugins and executable
plugins -- and in the form of using NADI as a library (Rust and
Python).

# Future Directions
As NADI is in a developing phase there are many things that could be
added. For example, optimization algorithms can be written that can
take input variables to change and output variables to optimize, the
algorithm could run a function to calculate the output variable while
changing the input variables to search for the optima. One possible
direction is to use the genetic algorithm to find the optimum
parameters for the network. The flattened network map is a good
representation of a gene for the genetic algorithm to work.

Although NADI is being explained as a river network analysis program,
it should also be useful in cases where you need a tree network
structure. Some examples are: water distribution systems (without
loops) where the input/output will be reversed, decision/policy trees,
dictionaries/lookup tables, file systems/databases, etc. Plugins for
the NADI system can be generated to facilitate analysis or
visualization of any of these tasks.

# References
