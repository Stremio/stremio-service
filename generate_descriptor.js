#!/usr/bin/env node

// This script generates an auto update descriptor for a given tag.
// It will get the files for the tag, calculate their hashes, and upload
// the descriptor to S3. In order to run it, you need to have s3cmd installed
// and configured with your AWS credentials.

const { exec } = require("child_process");
const { posix } = require('path');
const https = require('https');
const { createHash } = require('crypto');
const { tmpdir } = require('os');
const fs = require('fs');
const S3_BUCKET_PATH = "s3://stremio-artifacts/stremio-service/";
const S3_VERSIONS_PATH = "s3://stremio-artifacts/stremio-service/[versions]/";
const S3_HTTP_ENDPOINT = "https://s3-eu-west-1.amazonaws.com";
const OS_EXT = {
    '.exe': 'windows',
    '.dmg': 'macos',
}
const s3ls = path => {
    return new Promise((resolve, reject) => {
        exec(`s3cmd ls ${path}`, (err, stdout, stderr) => {
            const error = err || stderr && new Error(stderr);
            if (error) return reject(error);
            resolve(stdout);
        });
    });
}

// Parses a line from the S3 listing
const parseS3Listing = (line) => {
    const path = (line.match(/^\s*(?<date>\d{4}(?:-\d{2}){2} \d\d?:\d\d?)\s+\d+\s+(?<path>.*)\s*$/) || {}).groups;
    if (!path) return;
    return {
        name: posix.basename(path.path),
        url: S3_HTTP_ENDPOINT + path.path.replace(/^s3:\//, ''),
        os: OS_EXT[posix.extname(path.path)] || 'linux',
        date: new Date(path.date)
    }
}

// Downloads a file from S3 and returns a hash of it
const s3hash = (path) => {
    return new Promise((resolve, reject) => {
        const hash = createHash('sha256');
        https.get(path, (res) => {
            res.on('data', hash.update.bind(hash));
            res.on('end', () => {
                resolve(hash.digest('hex'));
            });
        }).on('error', reject);
    });
}

const getFilesForTag = async (tag) => {
    return Promise.all((await s3ls(S3_BUCKET_PATH + tag + '/').then((listing) => {
        console.info('Calculating hashes for files')
        return listing.split('\n').map(parseS3Listing);
    })).filter((file) => file).map(async (file) => {
        const checksum = await s3hash(file.url);
        console.info('The hash for', file.name, 'is', checksum);
        return {
            ...file,
            checksum
        };
    }));
}

// Writes the descriptor to a temporary file and returns the path
const writeTempFile = (tag, content) => {
    const path = `${tmpdir()}/${tag}.json`;
    fs.writeFileSync(path, content);
    console.info('Wrote descriptor to', path);
    return path;
}

// Upload the version descriptor to S3
const uploadDescriptor = (path) => {
    return new Promise((resolve, reject) => {
        exec(`s3cmd put --acl-public ${path} ${S3_VERSIONS_PATH}`, (err, stdout, stderr) => {
            const error = err || stderr && new Error(stderr);
            if (error) return reject(error);
            resolve(stdout);
        });
    });
}



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

const generateDescriptor = async (tag) => {
    if (!tag) {
        console.info('Obtaining the latest tag');
        tag = await s3ls(S3_BUCKET_PATH + 'v*').then((listing) => {
            // get the first line, remove the DIR prefix, and get the basename
            // which is the tag
            const first_path = listing.split('\n')[0].replace(/^\s+DIR\s+/, '');
            return posix.basename(first_path);
        })
    }
    console.info('Getting files for tag', tag);
    if (!tag) throw new Error('No tag found');
    const version = (tag.match(/^v(\d+\.\d+\.\d+).*$/) || [])[1];
    if (!version) throw new Error('No valid version found');
    const files = await getFilesForTag(tag);
    if (!files.length) throw new Error('No files found');
    const descriptor = {
        version,
        tag,
        released: files[0].date.toISOString(),
        files
    };
    const descriptor_path = writeTempFile(tag, JSON.stringify(descriptor, null, 2));
    console.info('Uploading descriptor to S3');
    try {
        await uploadDescriptor(descriptor_path);
    } finally {
        // Clean up the temporary file even if the upload fails
        console.info('Cleaning up');
        fs.unlinkSync(descriptor_path);
    }
    console.info('Done');
}

generateDescriptor(process.argv[2]).catch((err) => {
    console.error(err.message, err.message.length);
    process.exit(1);
});