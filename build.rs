fn main() {
    #[cfg(feature = "wsq")]
    {
        println!("cargo:rerun-if-changed=c_src");

        let mut build = cc::Build::new();
        build.include("c_src/include");
        build.define("__NBISLE__", None);
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
}
