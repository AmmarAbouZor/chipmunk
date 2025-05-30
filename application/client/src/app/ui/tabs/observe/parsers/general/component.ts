import { Component, ChangeDetectorRef, Input, AfterContentInit } from '@angular/core';
import { Ilc, IlcInterface } from '@env/decorators/component';
import { Initial } from '@env/decorators/initial';
import { ChangesDetector } from '@ui/env/extentions/changes';
import { State } from '../state';
import { State as GlobalState } from '../../state';

@Component({
    selector: 'app-el-parser-general',
    templateUrl: './template.html',
    styleUrls: ['./styles.less'],
    standalone: false,
})
@Initial()
@Ilc()
export class ParserGeneralConfiguration extends ChangesDetector implements AfterContentInit {
    @Input() state!: State;
    @Input() global!: GlobalState;

    constructor(cdRef: ChangeDetectorRef) {
        super(cdRef);
    }

    public ngAfterContentInit(): void {
        this.state.bind(this);
    }
}
export interface ParserGeneralConfiguration extends IlcInterface {}
