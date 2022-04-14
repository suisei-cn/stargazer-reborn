#!/bin/bash
cargo doc --no-deps

read -r -d '' html <<-EOF
<!DOCTYPE HTML>
 
<meta charset="UTF-8">
<meta http-equiv="refresh" content="1; url=/core">
 
<title>Page Redirection</title>
 
If you are not redirected automatically, follow the <a href='/core'>link to doc</a>
EOF

echo "$html" > target/doc/index.html
