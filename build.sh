#!/bin/bash
set -e
#//////////////////////////////////////////////////////////////////////
#//	Variable Setup
#//////////////////////////////////////////////////////////////////////
C_COLS=$(tput cols 2> /dev/null || echo "")
#//////////////////////////////////////////////////////////////////////
#//	Helper Functions
#//////////////////////////////////////////////////////////////////////
print () {
	echo -e "terra-base: $1"
}
break_scr() {
	[[ ! -z "$C_COLS" ]] && (printf '%*s\n' "${COLUMNS:-$(tput cols)}" '' | tr ' ' -) || echo "----------"
}
DIR_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
#//////////////////////////////////////////////////////////////////////
#//	Header
#//////////////////////////////////////////////////////////////////////
print "======================================="
print "terra ~           ;             ~ terra"
print " terra ~          ;;           ~ terra "
print "terra ~           ;';.          ~ terra"
print " terra ~          ;  ;;        ~ terra "
print "terra ~           ;   ;;        ~ terra"
print " terra ~          ;    ;;      ~ terra "
print "terra ~           ;    ;;       ~ terra"
print " terra ~          ;   ;'       ~ terra "
print "terra ~           ;  '          ~ terra"
print " terra ~      ,;;;,;           ~ terra "
print "terra ~       ;;;;;;            ~ terra"
print " terra ~      \`;;;;'          ~ terra "
print "======================================="
print
print "Prism build script says hello!"
print
break_scr
#//////////////////////////////////////////////////////////////////////
#//	Logic
#//////////////////////////////////////////////////////////////////////
print "Building prism-casset-token..."
cd $DIR_DIR/prism-casset-token && docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/rust-optimizer:0.11.4
