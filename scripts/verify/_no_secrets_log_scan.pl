#!/usr/bin/env perl
# Helper for verify_no_secrets.sh check (3). Scans one Rust source file
# for tracing/log macro calls that interpolate a whole envelope,
# payload, or a value named *proof/*secret/*token/*password, as opposed
# to a scoped scalar field pulled off one (e.g. `x = %envelope.field`,
# which is fine). String-literal contents (the human-readable message)
# are stripped before matching so a message like "...payload was not
# valid JSON..." does not trip the check. Prints one matching call per
# block found; prints nothing if the file is clean.
use strict;
use warnings;

my $file = shift or die "usage: $0 <file.rs>\n";
open(my $fh, '<', $file) or die "cannot open $file: $!\n";
local $/;
my $src = <$fh>;
close($fh);

# Strip '//' line comments (adequate for this codebase's style).
$src =~ s{//.*}{}g;

my $found = 0;
while ($src =~ /((?:tracing|log)::(?:trace|debug|info|warn|error)!\s*\((?:[^()]|\([^()]*\))*\))/gs) {
    my $call = $1;
    my $code_only = $call;
    # Drop string-literal contents so words inside the human-readable
    # message aren't mistaken for interpolated identifiers.
    $code_only =~ s/"(?:[^"\\]|\\.)*"//gs;
    if ($code_only =~ /(?<![.\w])[?%]?\b(envelope|payload|\w*proof|\w*secret|\w*token|\w*password)\b(?!\s*\.\w)/i) {
        print "MATCH:$file\n$call\n---END---\n";
        $found = 1;
    }
}
exit($found ? 1 : 0);
