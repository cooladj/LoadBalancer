import { Component, OnInit } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { HttpClient, HttpErrorResponse } from '@angular/common/http';
import {catchError, take} from 'rxjs/operators';
import { throwError } from 'rxjs';

interface PortResponse {
  message: string;
  current_numbers: number[];
}

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css'
})
export class AppComponent implements OnInit {
  title = 'LoadBalancerWeb';
  private readonly API_URL = 'http://localhost:8080/port';

  constructor(private http: HttpClient) {}

  ngOnInit() {
    const origin = (window.location.origin);
    console.log("this is the origin", origin);
    if (!origin) {
      console.error('No origin number available');
      return;
    }

    this.http.put<PortResponse>(this.API_URL, { origin: origin })
      .pipe(
        take(1),
        catchError(this.handleError)
      )
      .subscribe({
        next: (response) => {
          console.log('Port registered:', response.message);
        },
        error: (error) => {
          console.error('Failed to register origin:', error);
        }
      });
  }

  private handleError(error: HttpErrorResponse) {
    if (error.error?.error) {
      return throwError(() => error.error.error);
    }
    return throwError(() => 'Failed to register port');
  }
}
