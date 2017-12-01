#!/bin/bash
set -e
set -x

makeflags="-j$(nproc)"
cd ../../build
for builddir in 64 32; do
	pushd $builddir
	if [ -e dlls/winex11.drv/winex11.drv.so ]; then
		rm dlls/winex11.drv/winex11.drv.so
	fi
	make $makeflags -C dlls/winex11.drv liblol_hack.a
	make $makeflags dlls/winex11.drv
	if [ "$builddir" -eq "32" ]; then
		libdir=/usr/lib32/wine
	else
		libdir=/usr/lib/wine
	fi
	sudo cp dlls/winex11.drv/winex11.drv.so $libdir
	popd
done
