#set raw(syntaxes: "task.sublime-syntax")

= Syntax Highlighting using typst

``````task
# load the network and attributes
network load_file("data/ohio-river/ohio.network")
network gis_load_attrs("data/ohio-river/nid-uniq.gpkg", "nidId")
# identify dam and gages
node.is_usgs = NAME match "^[0-9]+";
node.is_dam = !is_usgs;
# recursively calculate number of gages and dams: Equation 1
node<inp>.ngage = int(is_usgs) + sum(inputs.ngage);
node<inp>.ndam = int(is_dam) + sum(inputs.ndam);
# recursively get year affected: Equation 2
node.dam_year = if (is_dam & yearCompleted?) { yearCompleted } else { nan };
node<inp>.dam_aff_yr = min_num(append(inputs.dam_aff_yr, dam_year));
``````
