import { Platform, getPlatform } from '@env/os/platform';

import * as os from 'os';

const NAME = 'chipmunk';
const HOOKS = {
    name: '<name>',
    version: '<version>',
    prefix: '<prefix>',
    platform: '<platform>',
};

const PATERN = `${HOOKS.name}${HOOKS.prefix}@${HOOKS.version}-${HOOKS.platform}-portable.tgz`;

export class ReleaseFile {
    public readonly filename: string;

    constructor(version: string, prefix = '') {
        const platform: Platform = getPlatform();
        if (platform === Platform.undefined) {
            throw new Error(`Fail to detect supported platform for (${os.platform()}).`);
        }
        this.filename = PATERN.replace(HOOKS.name, NAME)
            .replace(HOOKS.prefix, `${prefix === '' ? '' : `-${prefix}`}`)
            .replace(HOOKS.version, version)
            .replace(HOOKS.platform, platform);
    }

    public equal(filename: string): boolean {
        return filename === this.filename;
    }
}
