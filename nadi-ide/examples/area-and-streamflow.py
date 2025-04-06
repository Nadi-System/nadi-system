import sys
import pandas as pd

try:
    station = sys.argv[1]
except IndexError:
    print("Give station")
    exit(1)

# this is just an example, but you can load different csv files for
# each station and do a lot of analysis before sending those variables
# to nadi
df = pd.read_csv("node-attrs.csv", index_col="name")
# this is because pandas loads site numbers as integers
df.index = df.index.map(lambda s: f"{s:08}")

try:
    sf = df.loc[station, "streamflow"]
    ar = df.loc[station, "area"]
    # the prefix "nadi:var:" tells nadi to load them as node
    # attributes instead of ignoring the lines
    print(f"nadi:var:streamflow={sf}")
    print(f"nadi:var:area={ar}")
except KeyError:
    pass
