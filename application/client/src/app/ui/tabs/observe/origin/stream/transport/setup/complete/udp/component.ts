import { Component, ChangeDetectorRef, Input, OnDestroy, AfterContentInit } from '@angular/core';
import { Ilc, IlcInterface } from '@env/decorators/component';
import { Subject } from '@platform/env/subscription';
import { SetupBase } from '../../bases/udp/component';

@Component({
    selector: 'app-transport-udp',
    templateUrl: './template.html',
    styleUrls: ['./styles.less'],
    standalone: false,
})
@Ilc()
export class Setup extends SetupBase implements OnDestroy, AfterContentInit {
    @Input() public update?: Subject<void>;

    constructor(cdRef: ChangeDetectorRef) {
        super(cdRef);
    }

    public override ngAfterContentInit(): void {
        this.update !== undefined &&
            this.env().subscriber.register(
                this.update.subscribe(() => {
                    this.detectChanges();
                }),
            );
        super.ngAfterContentInit();
    }
}
export interface Setup extends IlcInterface {}
