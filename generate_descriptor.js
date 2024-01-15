#!/usr/bin/env node

// Copyright (C) 2017-2024 Smart Code OOD 203358507

// This script generates an auto update descriptor for a given tag.
// It will get the files for the tag, calculate their hashes, and upload
// the descriptor to S3. In order to run it, you need to have aws cli installed
// and configured with your AWS credentials.

const { exec } = require("child_process");
const { basename, posix } = require("path");
const https = require("https");
const { createHash } = require("crypto");
const { tmpdir } = require("os");
const fs = require("fs");
const S3_BUCKET_PATH = "s3://stremio-artifacts/stremio-service/";
const S3_VERSIONS_PATH = "s3://stremio-artifacts/stremio-service/[versions]/";
const S3_VERSIONS_RC_PATH =
    "s3://stremio-artifacts/stremio-service/[versions]/rc/";
const DOWNLOAD_ENDPOINT = "https://dl.strem.io/stremio-service/";
const OS_EXT = {
    ".exe": "windows",
    ".dmg": "macos",
};
const VERSION_REGEX = /^v(\d+\.\d+\.\d+).*$/;

const supportedArguments = Object.freeze({
    tag: {
        description:
            "The tag to generate the descriptor for. If not specified, the latest tag will be used. A tag must start with a v followed by the version number in semver format, followed by an optional suffix. For example: v1.2.3, v1.2.3-rc.1, v1.2.3-beta.2",
        parse: parseTagArgument,
    },
    force: {
        description: "Overwrite the descriptor if it already exists.",
        default: false,
        parse: parseBooleanArgument,
    },
    dry_run: {
        description:
            "Do not upload the descriptor to S3. Just print it to stdout. The descriptor is printed even if --quiet is set.",
        default: false,
        parse: parseBooleanArgument,
    },
    wait_all: {
        description:
            "By default at least one file is required to produce a descriptor. This flag will cause the script to exit (without error) if not all files are uploaded before generating the descriptor.",
        default: false,
        parse: parseBooleanArgument,
    },
    release: {
        description:
            "If this flag is set, the descriptor will be uploaded to the release path instead of the release candidate path.",
        default: false,
        parse: parseBooleanArgument,
    },
    quiet: {
        description: "Suppress all output except for errors.",
        default: false,
        parse: parseBooleanArgument,
    },
    help: {
        description: "Print this help message",
        parse: () => {
            usage();
            process.exit(0);
        },
    },
});

function parseBooleanArgument(value) {
    // Acceptable falsy values. Note that an empty string is considered truthy.
    // Thus --option and --option= will set option to true.
    return ["false", "0", "no", "off"].includes(value.toLowerCase())
        ? false
        : true;
}

function parseTagArgument(value) {
    if (value.match(VERSION_REGEX) !== null) return value;
}

function usage() {
    log(`Usage: ${basename(process.argv[1])} [options]`);
    log("Options:");
    Object.keys(supportedArguments).forEach((key) => {
        log(
            `  --${key.replace(/_/g, "-")}${typeof supportedArguments[key].default !== "undefined"
                ? " [default: " + supportedArguments[key].default.toString() + "]"
                : ""
            }`
        );
        log(`    ${supportedArguments[key].description}`);
    });
}

const parseArguments = () => {
    const args = Object.keys(supportedArguments).reduce((acc, key) => {
        if (typeof supportedArguments[key].default !== "undefined")
            acc[key] = supportedArguments[key].default;
        return acc;
    }, {});
    try {
        for (let i = 2; i < process.argv.length; i++) {
            const arg = process.argv[i];
            if (arg.startsWith("--")) {
                // Stop processing arguments after --
                if (arg.length === 2) break;
                const eq_position = arg.indexOf("=");
                const name_end = eq_position === -1 ? arg.length : eq_position;
                const name = arg.slice(2, name_end).replace(/-/g, "_");
                if (!supportedArguments[name])
                    throw new Error(`Unsupported argument ${arg}`);
                const value = supportedArguments[name].parse(arg.slice(name_end + 1));
                if (typeof value === "undefined")
                    throw new Error(
                        `Invalid value for argument --${name.replace(/_/g, "-")}`
                    );
                args[name] = value;
            }
        }
    } catch (e) {
        console.error(e.message);
        usage();
        process.exit(1);
    }
    return args;
};

const args = parseArguments();

function log(...params) {
    if (!args.quiet) console.info(...params);
}

const s3Cmd = (command_line) => {
    return new Promise((resolve, reject) => {
        exec(`aws s3 ${command_line}`, (err, stdout, stderr) => {
            const error = err || (stderr && new Error(stderr));
            if (error) return reject(error);
            resolve(stdout);
        });
    });
};

const s3Ls = (path) => s3Cmd(`ls --no-paginate ${path}`).catch(() => { });
const s3Cp = (src, dest) => s3Cmd(`cp --acl public-read ${src} ${dest}`);

// Downloads a file from S3 and returns a hash of it
const s3Hash = (path) => {
    return new Promise((resolve, reject) => {
        const hash = createHash("sha256");
        https
            .get(path, (res) => {
                res.on("data", hash.update.bind(hash));
                res.on("end", () => {
                    resolve(hash.digest("hex"));
                });
            })
            .on("error", reject);
    });
};

// Parses a line from the S3 listing
const parseS3Listing = (tag) => (line) => {
    const path = (
        line.match(
            /^\s*(?<date>\d{4}(?:-\d{2}){2} \d\d(?::\d\d){2})\s+\d+\s+(?<name>.*)\s*$/
        ) || {}
    ).groups;
    if (!path) return;
    const os = OS_EXT[posix.extname(path.name)];
    if (!os) return;
    return {
        name: path.name,
        url: `${DOWNLOAD_ENDPOINT + tag}/${path.name}`,
        os,
        date: new Date(path.date),
    };
};

const getFilesForTag = async (tag) =>
    (
        await s3Ls(S3_BUCKET_PATH + tag + "/").then((listing) => {
            log("Calculating hashes for files");
            return (listing || "").split("\n").map(parseS3Listing(tag));
        })
    ).filter((file) => file);

const calculateFileChecksums = async (files) =>
    Promise.all(
        files.map(async (file) => {
            const checksum = await s3Hash(file.url);
            log("The hash for", file.name, "is", checksum);
            return {
                ...file,
                checksum,
            };
        })
    );

// Generates the descriptor for a given tag
// If no tag is provided, it will get the latest tag
// An example descriptor:
//
// {
//     "version": "0.1.0",
//     "tag": "v0.1.0-new-setup",
//     "released": "2023-02-30T12:53:59.412Z",
//     "files": [
//       {
//         "name": "StremioServiceSetup.exe",
//         "url": "https://s3-eu-west-1.amazonaws.com/stremio-artifacts/stremio-service/v0.1.0-new-setup/StremioServiceSetup.exe",
//         "checksum": "0ff94905df4d94233d14f48ed68e31664a478a29204be4c7867c2389929c6ac3",
//         "os": "windows"
//       }
//     ]
//   }

const generateDescriptor = async (args) => {
    let tag = args.tag;
    if (!tag) {
        log("Obtaining the latest tag");
        tag = await s3Ls(S3_BUCKET_PATH).then((listing) => {
            // get the first line, remove the DIR prefix, and get the basename
            // which is the tag
            const first_path = listing.replace(/^\s+\w+\s+/gm, '').split('\n').find(line => line.match(VERSION_REGEX));
            return posix.basename(first_path);
        });
    }
    const desc_name = tag + ".json";
    const s3_rc_desc_path = S3_VERSIONS_RC_PATH + desc_name;
    const s3_dest_path = args.release
        ? S3_VERSIONS_PATH + desc_name
        : s3_rc_desc_path;
    if (!args.force && await s3Ls(s3_dest_path).then((listing) => !!listing)) {
        throw new Error(
            `${args.release ? "" : "RC "}Descriptor for tag ${tag} already exists`
        );
    }
    if (
        args.release &&
        !args.force &&
        (await s3Ls(s3_rc_desc_path).then((listing) => !!listing))
    ) {
        log(
            "Descriptor for tag",
            tag,
            "already exists in the RC folder. Moving it to the releases folder"
        );
        if (!args.dry_run) await s3Cp(s3_rc_desc_path, s3_dest_path);
        log("Done");
        return;
    }

    log("Getting files for tag", tag);
    if (!tag) throw new Error("No tag found");
    const version = (tag.match(VERSION_REGEX) || [])[1];
    if (!version) throw new Error("No valid version found");
    const file_listing = await getFilesForTag(tag);
    // We need at least one file to extract the release date
    if (!file_listing.length) throw new Error("No files found");
    if (args.wait_all && file_listing.length < Object.keys(OS_EXT).length) {
        log(
            `Not all files are uploaded yet. Rerun this script after ${Object.keys(OS_EXT).length - file_listing.length
            } more are uploaded`
        );
        return;
    }
    const files = await calculateFileChecksums(file_listing);
    const descriptor = {
        version,
        tag,
        released: file_listing[0].date.toISOString(),
        files,
    };
    const descriptor_text = JSON.stringify(descriptor, null, 2) + "\n";
    if (args.dry_run) {
        process.stdout.write(descriptor_text);
        return;
    }

    const descriptor_path = `${tmpdir()}/${desc_name}`;
    log("Writting descriptor to", descriptor_path);
    fs.writeFileSync(descriptor_path, descriptor_text);

    log(`Uploading ${args.release ? "" : "RC "}descriptor to S3`);
    try {
        await s3Cp(descriptor_path, s3_dest_path);
    } finally {
        // Clean up the temporary file even if the upload fails
        log("Cleaning up");
        fs.unlinkSync(descriptor_path);
    }
    log("Done");
};

generateDescriptor(args).catch((err) => {
    console.error(err.message);
    process.exit(1);
});
