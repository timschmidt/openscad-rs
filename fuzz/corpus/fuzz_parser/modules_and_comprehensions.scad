function twice(x) = x * 2;
module row(n = 3) {
    for (i = [0:n - 1]) translate([twice(i), 0, 0]) children();
}
values = [for (i = [0:3]) if (i != 2) each [i, i * i]];
row(len(values)) cube([1, 2, 3]);
