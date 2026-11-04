fn main() {
    #[cfg(feature = "wsq")]
    {
        println!("cargo:rerun-if-changed=c_src");

        let mut build = cc::Build::new();
        build.include("c_src/include");
        build.define("__NBISLE__", None);
        // NBIS exposes a generic global named `debug`; namespace it to avoid colliding with
        // the same legacy symbol in statically linked NFIQ2 distributions.
        build.define("debug", "wsq_nbis_debug");
        build.warnings(false);

        let files = [
            "c_src/wsq/cropcoeff.c",
            "c_src/wsq/decoder.c",
            "c_src/wsq/encoder.c",
            "c_src/wsq/globals.c",
            "c_src/wsq/huff.c",
            "c_src/wsq/ppi.c",
            "c_src/wsq/sd14util.c",
            "c_src/wsq/tableio.c",
            "c_src/wsq/tree.c",
            "c_src/wsq/util.c",
            "c_src/jpegl/decoder.c",
            "c_src/jpegl/encoder.c",
            "c_src/jpegl/huff.c",
            "c_src/jpegl/huftable.c",
            "c_src/jpegl/imgdat.c",
            "c_src/jpegl/ppi.c",
            "c_src/jpegl/sd4util.c",
            "c_src/jpegl/tableio.c",
            "c_src/jpegl/util.c",
            "c_src/fet/allocfet.c",
            "c_src/fet/extrfet.c",
            "c_src/fet/lkupfet.c",
            "c_src/fet/printfet.c",
            "c_src/fet/strfet.c",
            "c_src/fet/writefet.c",
            "c_src/fet/delfet.c",
            "c_src/fet/freefet.c",
            "c_src/fet/nistcom.c",
            "c_src/fet/readfet.c",
            "c_src/fet/updatfet.c",
            "c_src/util/computil.c",
            "c_src/util/syserr.c",
            "c_src/util/fatalerr.c",
            "c_src/ioutil/dataio.c",
            "c_src/ioutil/filesize.c",
        ];

        for file in &files {
            build.file(file);
        }

        build.compile("wsq_nbis");
    }

    #[cfg(feature = "nfiq2")]
    {
        use std::{env, path::PathBuf};

        println!("cargo:rerun-if-env-changed=NFIQ2_DIR");
        println!("cargo:rerun-if-changed=c_src/nfiq2_bridge.cpp");

        let prefix = env::var_os("NFIQ2_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                panic!("the nfiq2 feature requires NFIQ2_DIR to point to an NFIQ2 2.3 installation")
            });
        let include = prefix.join("include");
        let lib = prefix.join("lib");
        if !include.is_dir() || !lib.is_dir() {
            panic!(
                "NFIQ2_DIR must contain include/ and lib/ directories (got {})",
                prefix.display()
            );
        }

        cc::Build::new()
            .cpp(true)
            .file("c_src/nfiq2_bridge.cpp")
            .include(include)
            .warnings(false)
            .compile("fs80h_nfiq2_bridge");
        println!("cargo:rustc-link-search=native={}", lib.display());
        // Official NFIQ2 packages commonly ship `libnfiq2` as a static library. Link its
        // transitive FingerJet/OpenCV components explicitly when the package supplies them.
        // Dynamic package variants need only the first entry and harmlessly omit the rest.
        for library in [
            "nfiq2",
            "FRFXLL",
            "nfir",
            "opencv_ml",
            "opencv_imgproc",
            "opencv_core",
        ] {
            println!("cargo:rustc-link-lib={library}");
        }
        let opencv_third_party = lib.join("opencv4/3rdparty");
        if opencv_third_party.is_dir() {
            println!(
                "cargo:rustc-link-search=native={}",
                opencv_third_party.display()
            );
            println!("cargo:rustc-link-lib=static=zlib");
        }
        #[cfg(target_os = "macos")]
        {
            for framework in ["Accelerate", "AppKit", "OpenCL"] {
                println!("cargo:rustc-link-lib=framework={framework}");
            }
            // The NIST macOS package uses `@rpath` for FingerJet FX OSE.
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
        }
    }
}
