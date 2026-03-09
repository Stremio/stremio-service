#!/usr/bin/env node
const { readFileSync, writeFileSync } = require('fs');

function getGitVersion() {
    const { execSync } = require('child_process');
    let ver = execSync('git describe --tags --abbrev=0').toString().match(/^v(?<version>\d+\.\d+.\d+)/);
    if(ver === null || typeof ver.groups !== "object" || typeof ver.groups.version !== "string") return null;
    return ver.groups.version;
}
function getCargoVersion(str) {
    const psection = '\n[package]\n'
    const poffset = str.indexOf(psection)
    if (poffset === -1) return null;
    const voffset = str.indexOf('\nversion', poffset + psection.length);
    if (voffset === -1) return null;
    const vstart = str.indexOf('"', voffset) + 1;
    if (vstart === 0) return null;
    const vend = str.indexOf('"', vstart);

    return {vstart, vend, version: str.slice(vstart, vend) }
}

switch(process.argv[2]) {
    case 'check': {
        const cver = getCargoVersion(readFileSync('Cargo.toml').toString());
        const gver = getGitVersion();
        if(gver !== cver.version) {
            console.error(`Fatal error: the version in Cargo.toml (v${cver.version}) doesn't match latest tag (v${gver})!`);
            process.exit(1);
        }
        break;
    }
    case 'update': {
        let newVer = process.argv[3];
        if(! newVer) {
            const ghv = getGitVersion().split('.')
            const patch = (parseInt(ghv.pop(), 10) + 1).toString(10);
            ghv.push(patch);
            newVer = ghv.join('.');
            console.log(`WRNING: No new version provided. Using GH version + 1`)
        }
        const toml = readFileSync('Cargo.toml').toString();
        const cver = getCargoVersion(toml);
        if(cver.version === newVer) {
            console.log('Warinig: the new version is the same as the version in Cargo.toml')
        }
        const newtoml = toml.slice(0, cver.vstart) + newVer + toml.slice(cver.vend)
        writeFileSync('Cargo.toml', newtoml);
        console.log(`Cargo.toml updated to v${newVer}`);
        console.log('Changes can be upstreamed by the following commands:\n');
        console.log('git add Cargo.toml');
        console.log(`git commit -m "Version updated to v${newVer}"`);
        console.log(`git tag -a v${newVer} -m "Release of v${newVer}"`);
        console.log('git push');
        console.log('git push --tags');

    }
    default: {
        console.log(`usage: ${process.argv[0]} check|update ver`)
    }
}


