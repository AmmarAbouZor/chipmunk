import { Component, ChangeDetectorRef, Input, AfterContentInit } from '@angular/core';
import { Ilc, IlcInterface } from '@env/decorators/component';
import { Initial } from '@env/decorators/initial';
import { ChangesDetector } from '@ui/env/extentions/changes';
import { bytesToStr } from '@env/str';
import { State } from './state';
import { Observe } from '@platform/types/observe';

@Component({
    selector: 'app-el-someip-general',
    templateUrl: './template.html',
    styleUrls: ['./styles.less'],
    standalone: false,
})
@Initial()
@Ilc()
export class SomeIpGeneralConfiguration extends ChangesDetector implements AfterContentInit {
    @Input() observe!: Observe;

    protected state!: State;

    public bytesToStr = bytesToStr;

    constructor(cdRef: ChangeDetectorRef) {
        super(cdRef);
    }

    public ngAfterContentInit(): void {
        this.state = new State(this.observe);
        this.state.bind(this);
    }
}
export interface SomeIpGeneralConfiguration extends IlcInterface {}
