import { Component, ChangeDetectorRef, Input, AfterContentInit } from '@angular/core';
import { Ilc, IlcInterface } from '@env/decorators/component';
import { Initial } from '@env/decorators/initial';
import { ChangesDetector } from '@ui/env/extentions/changes';
import { State } from '../state';

@Component({
    selector: 'app-el-parser-extra',
    templateUrl: './template.html',
    styleUrls: ['./styles.less'],
    standalone: false,
})
@Initial()
@Ilc()
export class ParserExtraConfiguration extends ChangesDetector implements AfterContentInit {
    @Input() state!: State;

    constructor(cdRef: ChangeDetectorRef) {
        super(cdRef);
    }

    public ngAfterContentInit(): void {
        this.state.bind(this);
    }
}
export interface ParserExtraConfiguration extends IlcInterface {}
