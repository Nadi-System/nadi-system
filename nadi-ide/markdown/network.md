# Network View Help

The network visualization shows the network with the nodes. Edit the template above to show the node label using different attributes. Refer to the documentation on [`String Templates`](https://nadi-system.github.io/devref/string-templates.html) for how to write templates.

You can click on the nodes to see the node's (or outside the node to see network's) attributes. the visualization is done using some special attributes that you can assign

## Node attributes for Visuals

- nodecolor: Color of the node shape fill
- linecolor: Color of the line to output
- textcolor: color of the node label/row
- linewidth: linewidth of the line to output
- nodesize: size of the node
- nodeshape: shape of the node

Colors can be (inferred based on type):
- Monotone gray (integer values: 0-255),
- Named color (string like: red, blue, etc),
- RGB values (array of 3 elements: 0-1)

Node Shape can be:
- Box, Square
- Circle
- Triangle
- Ellipse
- Rectangle

size and width should be float.
