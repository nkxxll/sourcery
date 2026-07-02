from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path
from typing import Literal
from urllib.parse import quote

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

import correlation


LEVEL_ALIASES = {
    "versions": "version",
    "version": "version",
    "files": "file",
    "file": "file",
    "functions": "function",
    "function": "function",
}
LOC_METRIC_BY_LEVEL = {
    "version": correlation.LOC_METRIC_VERSION,
    "file": correlation.LOC_METRIC_FILE,
    "function": correlation.LOC_METRIC_FUNCTION,
}
METRIC_ALIASES = {
    ("version", "cyclomatic complexity"): "total_cyclomatic",
    ("file", "cyclomatic complexity"): "total_cyclomatic",
    ("function", "cyclomatic complexity"): "cyclomatic",
}
DEFAULT_SAMPLE_COLUMNS = [
    "metric_key",
    "metric_label",
    "value",
    "codebase_id",
    "programming_language",
    "version_id",
    "version_number",
    "sample_number",
    "codebase_name",
    "file_path",
    "function_name",
    "function_start_line",
    "function_end_line",
]
GIT_REF_COLUMNS = ["commit_sha", "commit_hash", "git_sha", "git_ref", "ref"]
SOURCERY_DB = Path(__file__).resolve().parents[1] / "target" / "debug" / "sourcery-db"
FILE_HALSTEAD_OPERATOR_OPERAND_METRICS = [
    "total_halstead_operators",
    "total_halstead_operands",
    "total_halstead_unique_operators",
    "total_halstead_unique_operands",
]
FUNCTION_HALSTEAD_OPERATOR_OPERAND_METRICS = [
    "halstead_operators",
    "halstead_operands",
    "halstead_unique_operators",
    "halstead_unique_operands",
]
_VERSION_REF_CACHE: dict[tuple[str, int], str | None] = {}


class Language(StrEnum):
    GO = "Golang"
    OCAML = "Ocaml"


@dataclass
class MetricsData:
    """Interactive wrapper around the raw and version metric CSV files."""

    raw_metrics: pd.DataFrame
    version_metrics: pd.DataFrame
    root: Path = Path(".")

    @property
    def all_metrics(self) -> pd.DataFrame:
        return pd.concat([self.raw_metrics, self.version_metrics], ignore_index=True)

    def metric_names(self, level: str | None = None) -> pd.DataFrame:
        """Return known metrics with keys, labels, levels, and row counts."""
        df = self._metrics_for_level(level) if level else self.all_metrics
        label_col = (
            "metric_label" if "metric_label" in df.columns else correlation.METRIC_KEY
        )
        return (
            df.groupby(
                [correlation.METRIC_LEVEL_COL, correlation.METRIC_KEY], dropna=False
            )
            .agg(
                metric_label=(label_col, "first"), rows=(correlation.VALUE_COL, "size")
            )
            .reset_index()
            .sort_values([correlation.METRIC_LEVEL_COL, correlation.METRIC_KEY])
        )

    def sample(
        self,
        count: int = 20,
        metric: str = "cyclomatic",
        level: str = "function",
        value: float | None = None,
        op: Literal["==", "=", "!=", "<", "<=", ">", ">="] = "==",
        language: str | Language | None = None,
        version: int | Literal["all"] | None = None,
        random: bool = False,
        seed: int | None = 0,
        distribute_by_project: bool = False,
        file_extension: str | None = None,
        columns: list[str] | None = None,
    ) -> pd.DataFrame:
        """Return rows matching a metric query.

        Example: data.sample(20, "cyclomatic complexity", "functions", value=2)
        """
        df = self._metrics_for_level(level)
        metric_key = self.resolve_metric(metric, level=level)
        df = df[df[correlation.METRIC_KEY] == metric_key].copy()

        if language is not None:
            language_query = _normalize(_language_value(language))
            df = df[df[correlation.LANGUAGE_COL].map(_normalize) == language_query]

        df = _filter_file_extension(df, file_extension)

        if version is not None and version != "all":
            version_col = "version_number" if "version_number" in df.columns else correlation.VERSION_COL
            df[version_col] = pd.to_numeric(df[version_col], errors="coerce")
            df = df[df[version_col] == version]

        if value is not None:
            df = _filter_value(df, value, op)

        if distribute_by_project:
            df = _sample_evenly_by_project(df, count, random=random, seed=seed)
        elif random:
            count = min(count, len(df))
            df = df.sample(n=count, random_state=seed)
        else:
            df = df.head(count)

        selected_columns = columns or DEFAULT_SAMPLE_COLUMNS
        selected_columns = [col for col in selected_columns if col in df.columns]
        return df[selected_columns].reset_index(drop=True)

    def sample_median_by_language(
        self,
        count: int = 20,
        metric: str = "function_length",
        level: str = "function",
        input_index: int = 10,
        languages: list[str | Language] | None = None,
        random: bool = False,
        seed: int | None = 0,
        file_extension: str | None = None,
        columns: list[str] | None = None,
    ) -> pd.DataFrame:
        """Sample rows at the summary median for each language/input index."""
        level_name = normalize_level(level)
        metric_key = self.resolve_metric(metric, level=level_name)
        summary = pd.read_csv(self.root / f"metrics_by_language_{level_name}.csv")
        median_rows = summary[
            (summary[correlation.METRIC_LEVEL_COL] == level_name)
            & (summary[correlation.METRIC_KEY] == metric_key)
            & (summary["input_index"] == input_index)
        ].copy()

        if languages is not None:
            language_queries = {_normalize(_language_value(item)) for item in languages}
            median_rows = median_rows[
                median_rows[correlation.LANGUAGE_COL].map(_normalize).isin(language_queries)
            ]

        samples = []
        for _, row in median_rows.sort_values(correlation.LANGUAGE_COL).iterrows():
            language = row[correlation.LANGUAGE_COL]
            median = row["median"]
            sample = self.sample(
                count=count,
                metric=metric_key,
                level=level_name,
                value=median,
                language=language,
                version=input_index,
                random=random,
                seed=seed,
                file_extension=file_extension,
                columns=columns,
            )
            sample.insert(0, "median", median)
            samples.append(sample)

        if not samples:
            return pd.DataFrame(columns=columns or DEFAULT_SAMPLE_COLUMNS)
        return pd.concat(samples, ignore_index=True)

    def plot_model(
        self,
        metric: str,
        level: str = "version",
        language: str | Language | None = None,
        model: str | None = None,
        loc_metric: str | None = None,
        file_extension: str | None = None,
        ax: plt.Axes | None = None,
    ) -> plt.Axes:
        """Plot real data and a fitted model for one metric/language/level."""
        level_name = normalize_level(level)
        df = self._metrics_for_level(level_name)
        metric_key = self.resolve_metric(metric, level=level_name)
        loc_metric = loc_metric or LOC_METRIC_BY_LEVEL[level_name]

        if language is None:
            languages = sorted(df[correlation.LANGUAGE_COL].dropna().unique())
            if len(languages) != 1:
                raise ValueError(
                    "language is required when data contains multiple languages: "
                    + ", ".join(map(str, languages))
                )
            language = str(languages[0])
        else:
            language = _language_value(language)

        language_df = df[df[correlation.LANGUAGE_COL] == language]
        language_df = _filter_file_extension(language_df, file_extension)
        if language_df.empty:
            raise ValueError(
                f"no {level_name} metric data found for language {language!r}"
            )

        paired = correlation.build_metric_pairs(
            language_df, level_name, loc_metric, metric_key
        )
        paired = paired[
            np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
        ].copy()
        if paired.empty:
            raise ValueError(
                f"no paired {loc_metric!r} and {metric_key!r} rows found for {language!r}"
            )

        model_results = correlation.linearized_model_results(paired)
        model_name = model or _best_model_name(model_results)
        model_result = {
            key.removeprefix(f"{model_name}_"): value
            for key, value in model_results.items()
            if key.startswith(f"{model_name}_")
        }
        if pd.isna(model_result.get("r2")):
            raise ValueError(f"could not fit {model_name!r} model for {metric_key!r}")

        ax = ax or plt.subplots(figsize=(8, 5))[1]
        loc = paired["loc"].to_numpy(dtype=float)
        metric_values = paired["metric_value"].to_numpy(dtype=float)
        order = np.argsort(loc)
        loc = loc[order]
        metric_values = metric_values[order]

        curve_x = np.linspace(loc.min(), loc.max(), 300)
        curve_y = correlation.model_prediction(
            model_name, curve_x, model_result["slope"], model_result["intercept"]
        )
        finite_curve = np.isfinite(curve_x) & np.isfinite(curve_y)

        ax.scatter(loc, metric_values, label="real data", zorder=3)
        ax.plot(
            curve_x[finite_curve],
            curve_y[finite_curve],
            label=f"{model_name} model, R^2={model_result['r2']:.3f}",
            color="#d62728",
            linewidth=2,
        )
        ax.set_title(f"{language}: {metric_key} from {loc_metric}")
        ax.set_xlabel(loc_metric)
        ax.set_ylabel(metric_key)
        ax.grid(True, alpha=0.3)
        ax.legend()
        ax.figure.tight_layout()
        return ax

    def plot_models(
        self,
        metric: str,
        level: str = "version",
        languages: list[str | Language] | None = None,
        model: str | None = None,
        file_extension: str | None = None,
    ) -> plt.Figure:
        """Plot a metric model for each requested language."""
        level_name = normalize_level(level)
        df = self._metrics_for_level(level_name)
        languages = languages or sorted(df[correlation.LANGUAGE_COL].dropna().unique())
        languages = [_language_value(language) for language in languages]
        fig, axes = plt.subplots(len(languages), 1, figsize=(8, 4 * len(languages)))
        axes = np.atleast_1d(axes)
        for ax, language in zip(axes, languages, strict=False):
            self.plot_model(
                metric,
                level_name,
                language,
                model=model,
                file_extension=file_extension,
                ax=ax,
            )
        return fig

    def plot_file_spread_to_loc(
        self,
        metrics: list[str] | None = None,
        language: str | Language | None = None,
        languages: list[str | Language] | None = None,
        alpha: float = 0.25,
        s: float = 10,
        log: bool = False,
        file_extension: str | None = None,
    ) -> plt.Figure:
        """Plot file-level metric spreads against file lines_of_code."""
        if language is not None and languages is not None:
            raise ValueError("use either language=... or languages=..., not both")

        metrics = metrics or FILE_HALSTEAD_OPERATOR_OPERAND_METRICS
        file_metrics = self._metrics_for_level("file")
        file_metrics = _filter_file_extension(file_metrics, file_extension)
        if language is not None:
            languages = [language]

        languages = [_language_value(item) for item in languages] if languages else None

        resolved_metrics = [
            self.resolve_metric(metric, level="file") for metric in metrics
        ]
        if languages:
            return self._plot_file_spread_grid(
                file_metrics,
                resolved_metrics,
                languages,
                alpha=alpha,
                s=s,
                log=log,
            )

        fig, axes = plt.subplots(
            len(resolved_metrics),
            1,
            figsize=(8, 3.5 * len(resolved_metrics)),
            sharex=True,
        )
        axes = np.atleast_1d(axes)

        for ax, metric_key in zip(axes, resolved_metrics, strict=False):
            paired = correlation.build_metric_pairs(
                file_metrics, "file", correlation.LOC_METRIC_FILE, metric_key
            )
            paired = paired[
                np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
            ]
            ax.scatter(paired["loc"], paired["metric_value"], alpha=alpha, s=s)
            ax.set_ylabel(metric_key)
            ax.grid(True, alpha=0.25)
            if log:
                ax.set_xscale("log")
                ax.set_yscale("log")

        title = "File metric spread vs lines_of_code"
        if language is not None:
            title += f" ({_language_value(language)})"
        axes[0].set_title(title)
        axes[-1].set_xlabel(correlation.LOC_METRIC_FILE)
        fig.tight_layout()
        return fig

    def plot_go_ocaml_file_spread_to_loc(
        self,
        metrics: list[str] | None = None,
        alpha: float = 0.25,
        s: float = 10,
        log: bool = False,
        file_extension: str | None = None,
    ) -> plt.Figure:
        """Plot Go and OCaml file-level Halstead spreads side by side."""
        return self.plot_file_spread_to_loc(
            metrics=metrics,
            languages=[Language.GO, Language.OCAML],
            alpha=alpha,
            s=s,
            log=log,
            file_extension=file_extension,
        )

    def plot_function_spread_to_loc(
        self,
        metrics: list[str] | None = None,
        language: str | Language | None = None,
        languages: list[str | Language] | None = None,
        alpha: float = 0.25,
        s: float = 10,
        log: bool = False,
        file_extension: str | None = None,
    ) -> plt.Figure:
        """Plot function-level metric spreads against function_length."""
        return self._plot_spread_to_loc(
            level="function",
            loc_metric=correlation.LOC_METRIC_FUNCTION,
            default_metrics=FUNCTION_HALSTEAD_OPERATOR_OPERAND_METRICS,
            title="Function metric spread vs function_length",
            metrics=metrics,
            language=language,
            languages=languages,
            alpha=alpha,
            s=s,
            log=log,
            file_extension=file_extension,
        )

    def plot_go_ocaml_function_spread_to_loc(
        self,
        metrics: list[str] | None = None,
        alpha: float = 0.25,
        s: float = 10,
        log: bool = False,
        file_extension: str | None = None,
    ) -> plt.Figure:
        """Plot Go and OCaml function-level Halstead spreads side by side."""
        return self.plot_function_spread_to_loc(
            metrics=metrics,
            languages=[Language.GO, Language.OCAML],
            alpha=alpha,
            s=s,
            log=log,
            file_extension=file_extension,
        )

    def _plot_spread_to_loc(
        self,
        level: str,
        loc_metric: str,
        default_metrics: list[str],
        title: str,
        metrics: list[str] | None,
        language: str | Language | None,
        languages: list[str | Language] | None,
        alpha: float,
        s: float,
        log: bool,
        file_extension: str | None,
    ) -> plt.Figure:
        if language is not None and languages is not None:
            raise ValueError("use either language=... or languages=..., not both")

        metrics = metrics or default_metrics
        level_metrics = self._metrics_for_level(level)
        level_metrics = _filter_file_extension(level_metrics, file_extension)
        if language is not None:
            languages = [language]

        languages = [_language_value(item) for item in languages] if languages else None
        resolved_metrics = [
            self.resolve_metric(metric, level=level) for metric in metrics
        ]

        if languages:
            return self._plot_spread_grid(
                level_metrics,
                level,
                loc_metric,
                resolved_metrics,
                languages,
                title=title,
                alpha=alpha,
                s=s,
                log=log,
            )

        fig, axes = plt.subplots(
            len(resolved_metrics),
            1,
            figsize=(8, 3.5 * len(resolved_metrics)),
            sharex=True,
        )
        axes = np.atleast_1d(axes)

        for ax, metric_key in zip(axes, resolved_metrics, strict=False):
            paired = correlation.build_metric_pairs(
                level_metrics, level, loc_metric, metric_key
            )
            paired = paired[
                np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
            ]
            if log:
                paired = paired[(paired["loc"] > 0) & (paired["metric_value"] > 0)]
            ax.scatter(paired["loc"], paired["metric_value"], alpha=alpha, s=s)
            ax.set_ylabel(metric_key)
            ax.grid(True, alpha=0.25)
            if log:
                ax.set_xscale("log")
                ax.set_yscale("log")

        axes[0].set_title(title)
        axes[-1].set_xlabel(loc_metric)
        fig.tight_layout()
        return fig

    def _plot_file_spread_grid(
        self,
        file_metrics: pd.DataFrame,
        metrics: list[str],
        languages: list[str],
        alpha: float,
        s: float,
        log: bool,
    ) -> plt.Figure:
        fig, axes = plt.subplots(
            len(metrics),
            len(languages),
            figsize=(5 * len(languages), 3.5 * len(metrics)),
            sharex=True,
            sharey="row",
            squeeze=False,
        )

        for row_index, metric_key in enumerate(metrics):
            for col_index, language in enumerate(languages):
                ax = axes[row_index, col_index]
                language_query = _normalize(language)
                language_metrics = file_metrics[
                    file_metrics[correlation.LANGUAGE_COL].map(_normalize)
                    == language_query
                ]
                paired = correlation.build_metric_pairs(
                    language_metrics, "file", correlation.LOC_METRIC_FILE, metric_key
                )
                paired = paired[
                    np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
                ]
                if log:
                    paired = paired[(paired["loc"] > 0) & (paired["metric_value"] > 0)]

                ax.scatter(paired["loc"], paired["metric_value"], alpha=alpha, s=s)
                ax.grid(True, alpha=0.25)
                ax.set_title(f"{language}: {metric_key}")
                if col_index == 0:
                    ax.set_ylabel(metric_key)
                if row_index == len(metrics) - 1:
                    ax.set_xlabel(correlation.LOC_METRIC_FILE)
                if log:
                    ax.set_xscale("log")
                    ax.set_yscale("log")

        fig.suptitle("File metric spread vs lines_of_code", y=1.0)
        fig.tight_layout()
        return fig

    def _plot_spread_grid(
        self,
        level_metrics: pd.DataFrame,
        level: str,
        loc_metric: str,
        metrics: list[str],
        languages: list[str],
        title: str,
        alpha: float,
        s: float,
        log: bool,
    ) -> plt.Figure:
        fig, axes = plt.subplots(
            len(metrics),
            len(languages),
            figsize=(5 * len(languages), 3.5 * len(metrics)),
            sharex=True,
            sharey="row",
            squeeze=False,
        )

        for row_index, metric_key in enumerate(metrics):
            for col_index, language in enumerate(languages):
                ax = axes[row_index, col_index]
                language_query = _normalize(language)
                language_metrics = level_metrics[
                    level_metrics[correlation.LANGUAGE_COL].map(_normalize)
                    == language_query
                ]
                paired = correlation.build_metric_pairs(
                    language_metrics, level, loc_metric, metric_key
                )
                paired = paired[
                    np.isfinite(paired["loc"]) & np.isfinite(paired["metric_value"])
                ]
                if log:
                    paired = paired[(paired["loc"] > 0) & (paired["metric_value"] > 0)]

                ax.scatter(paired["loc"], paired["metric_value"], alpha=alpha, s=s)
                ax.grid(True, alpha=0.25)
                ax.set_title(f"{language}: {metric_key}")
                if col_index == 0:
                    ax.set_ylabel(metric_key)
                if row_index == len(metrics) - 1:
                    ax.set_xlabel(loc_metric)
                if log:
                    ax.set_xscale("log")
                    ax.set_yscale("log")

        fig.suptitle(title, y=1.0)
        fig.tight_layout()
        return fig

    def correlations(
        self,
        level: str = "version",
        file_extension: str | None = None,
    ) -> pd.DataFrame:
        """Calculate the same correlation/model table as correlation.py for a level."""
        level_name = normalize_level(level)
        df = self._metrics_for_level(level_name)
        df = _filter_file_extension(df, file_extension)
        result = correlation.spearman_for_metric_language(
            df, LOC_METRIC_BY_LEVEL[level_name], level_name
        )
        return correlation.metric_selection_table(result)

    def with_github_urls(
        self,
        df: pd.DataFrame,
        ref: str | None = None,
        resolve_refs: bool = False,
        strict: bool = False,
        sourcery_db: str | Path = SOURCERY_DB,
        url_col: str = "github_url",
    ) -> pd.DataFrame:
        """Return df with GitHub URLs derived from codebase_name and file locations."""
        return with_github_urls(
            df,
            ref=ref,
            resolve_refs=resolve_refs,
            strict=strict,
            sourcery_db=sourcery_db,
            url_col=url_col,
        )

    def sample_urls(
        self,
        count: int = 20,
        metric: str = "cyclomatic",
        level: str = "function",
        value: float | None = None,
        op: Literal["==", "=", "!=", "<", "<=", ">", ">="] = "==",
        language: str | Language | None = None,
        version: int | Literal["all"] | None = None,
        random: bool = False,
        seed: int | None = 0,
        distribute_by_project: bool = False,
        file_extension: str | None = None,
        columns: list[str] | None = None,
        url_col: str = "github_url",
    ) -> pd.DataFrame:
        """Sample metric rows, add exact GitHub URLs, print them, and return rows."""
        sample_columns = _columns_for_url_generation(columns)
        rows = self.sample(
            count=count,
            metric=metric,
            level=level,
            value=value,
            op=op,
            language=language,
            version=version,
            random=random,
            seed=seed,
            distribute_by_project=distribute_by_project,
            file_extension=file_extension,
            columns=sample_columns,
        )
        rows = self.with_github_urls(
            rows, resolve_refs=True, strict=True, url_col=url_col
        )
        print_urls(rows, url_col=url_col)
        return rows

    def sample_median_urls_by_language(
        self,
        count: int = 20,
        metric: str = "function_length",
        level: str = "function",
        input_index: int = 10,
        languages: list[str | Language] | None = None,
        random: bool = False,
        seed: int | None = 0,
        file_extension: str | None = None,
        columns: list[str] | None = None,
        url_col: str = "github_url",
    ) -> pd.DataFrame:
        """Sample median rows per language, add GitHub URLs, print them, and return rows."""
        sample_columns = _columns_for_url_generation(columns)
        rows = self.sample_median_by_language(
            count=count,
            metric=metric,
            level=level,
            input_index=input_index,
            languages=languages,
            random=random,
            seed=seed,
            file_extension=file_extension,
            columns=sample_columns,
        )
        rows = self.with_github_urls(
            rows, resolve_refs=True, strict=True, url_col=url_col
        )
        print_urls(rows, url_col=url_col)
        return rows

    def file_loc_change_counts(
        self,
        change_counts: pd.DataFrame | None = None,
        language: str | Language | None = None,
        version: int | Literal["latest", "all"] = "latest",
        file_extension: str | None = None,
    ) -> pd.DataFrame:
        """Join file-level lines_of_code to file change counts."""
        return self._file_metric_change_counts(
            correlation.LOC_METRIC_FILE,
            "lines_of_code",
            change_counts=change_counts,
            language=language,
            version=version,
            file_extension=file_extension,
        )

    def file_cyclomatic_change_counts(
        self,
        change_counts: pd.DataFrame | None = None,
        language: str | Language | None = None,
        version: int | Literal["latest", "all"] = "latest",
        file_extension: str | None = None,
    ) -> pd.DataFrame:
        """Join file-level total_cyclomatic to file change counts."""
        return self._file_metric_change_counts(
            "total_cyclomatic",
            "total_cyclomatic",
            change_counts=change_counts,
            language=language,
            version=version,
            file_extension=file_extension,
        )

    def _file_metric_change_counts(
        self,
        metric_key: str,
        value_col: str,
        change_counts: pd.DataFrame | None,
        language: str | Language | None,
        version: int | Literal["latest", "all"],
        file_extension: str | None,
    ) -> pd.DataFrame:
        change_counts = (
            change_counts
            if change_counts is not None
            else load_file_change_counts(self.root)
        )
        metric_rows = self.raw_metrics[
            (self.raw_metrics[correlation.METRIC_LEVEL_COL] == "file")
            & (self.raw_metrics[correlation.METRIC_KEY] == metric_key)
        ].copy()
        metric_rows = _filter_file_extension(metric_rows, file_extension)
        change_counts = _filter_file_extension(change_counts, file_extension)

        if language is not None:
            language_query = _normalize(_language_value(language))
            metric_rows = metric_rows[
                metric_rows[correlation.LANGUAGE_COL].map(_normalize) == language_query
            ]
            if correlation.LANGUAGE_COL in change_counts.columns:
                change_counts = change_counts[
                    change_counts[correlation.LANGUAGE_COL].map(_normalize)
                    == language_query
                ]

        metric_codebases = set(metric_rows["codebase_id"].dropna())
        change_count_codebases = set(change_counts["codebase_id"].dropna())
        missing_from_change_counts = sorted(metric_codebases - change_count_codebases)
        missing_from_metric = sorted(change_count_codebases - metric_codebases)
        if missing_from_change_counts or missing_from_metric:
            scope = f" for {_language_value(language)}" if language is not None else ""
            details = []
            if missing_from_change_counts:
                details.append(
                    "missing from file change counts: "
                    + ", ".join(missing_from_change_counts)
                )
            if missing_from_metric:
                details.append(
                    f"missing from file {metric_key} metrics: "
                    + ", ".join(missing_from_metric)
                )
            raise ValueError("codebase mismatch" + scope + "; " + "; ".join(details))

        metric_rows = metric_rows.rename(columns={correlation.VALUE_COL: value_col})
        metric_rows[value_col] = pd.to_numeric(metric_rows[value_col], errors="coerce")
        metric_rows = metric_rows.dropna(subset=["codebase_id", "file_path", value_col])

        version_col = (
            "version_number"
            if "version_number" in metric_rows.columns
            else correlation.VERSION_COL
        )
        if version == "latest":
            metric_rows[version_col] = pd.to_numeric(
                metric_rows[version_col], errors="coerce"
            )
            metric_rows = metric_rows.sort_values(version_col).drop_duplicates(
                ["codebase_id", "file_path"], keep="last"
            )
        elif version != "all":
            metric_rows[version_col] = pd.to_numeric(
                metric_rows[version_col], errors="coerce"
            )
            metric_rows = metric_rows[metric_rows[version_col] == version]

        cols = [
            col
            for col in [
                "codebase_id",
                "codebase_name",
                correlation.LANGUAGE_COL,
                "version_id",
                "version_number",
                "sample_number",
                "file_path",
                value_col,
            ]
            if col in metric_rows.columns
        ]
        joined = metric_rows[cols].merge(
            change_counts[["codebase_id", "file_path", "change_count"]],
            on=["codebase_id", "file_path"],
            how="inner",
        )
        joined["change_count"] = pd.to_numeric(joined["change_count"], errors="coerce")
        return joined.dropna(subset=[value_col, "change_count"]).reset_index(drop=True)

    def file_loc_change_correlation(
        self,
        change_counts: pd.DataFrame | None = None,
        language: str | Language | None = None,
        version: int | Literal["latest", "all"] = "latest",
        file_extension: str | None = None,
    ) -> dict[str, float]:
        """Return Pearson and Spearman correlation between LOC and change count."""
        paired = self.file_loc_change_counts(
            change_counts=change_counts,
            language=language,
            version=version,
            file_extension=file_extension,
        )
        return loc_change_correlation(paired)

    def file_cyclomatic_change_correlation(
        self,
        change_counts: pd.DataFrame | None = None,
        language: str | Language | None = None,
        version: int | Literal["latest", "all"] = "latest",
        file_extension: str | None = None,
    ) -> dict[str, float]:
        """Return Pearson and Spearman correlation between cyclomatic and change count."""
        paired = self.file_cyclomatic_change_counts(
            change_counts=change_counts,
            language=language,
            version=version,
            file_extension=file_extension,
        )
        return metric_change_correlation(paired, "total_cyclomatic")

    def plot_file_loc_change_counts(
        self,
        change_counts: pd.DataFrame | None = None,
        language: str | Language | None = None,
        version: int | Literal["latest", "all"] = "latest",
        log: bool = False,
        alpha: float = 0.25,
        s: float = 10,
        file_extension: str | None = None,
    ) -> plt.Axes:
        """Plot file lines_of_code against lifetime file change_count."""
        paired = self.file_loc_change_counts(
            change_counts=change_counts,
            language=language,
            version=version,
            file_extension=file_extension,
        )
        ax = plt.subplots(figsize=(8, 5))[1]
        plot_data = paired
        if log:
            plot_data = paired[
                (paired["lines_of_code"] > 0) & (paired["change_count"] > 0)
            ]
        ax.scatter(
            plot_data["lines_of_code"], plot_data["change_count"], alpha=alpha, s=s
        )
        if log:
            ax.set_xscale("log")
            ax.set_yscale("log")
        stats = loc_change_correlation(plot_data)
        title = (
            f"File change count vs lines_of_code, version={version} (n={stats['n']})"
        )
        if language is not None:
            title += f" - {_language_value(language)}"
        title += f"\nSpearman r={stats['spearman_r']:.3f}, Pearson r={stats['pearson_r']:.3f}"
        ax.set_title(title)
        ax.set_xlabel("lines_of_code")
        ax.set_ylabel("change_count")
        ax.grid(True, alpha=0.25)
        ax.figure.tight_layout()
        return ax

    def plot_file_cyclomatic_change_counts(
        self,
        change_counts: pd.DataFrame | None = None,
        language: str | Language | None = None,
        version: int | Literal["latest", "all"] = "latest",
        log: bool = False,
        alpha: float = 0.25,
        s: float = 10,
        file_extension: str | None = None,
    ) -> plt.Axes:
        """Plot file total_cyclomatic against lifetime file change_count."""
        paired = self.file_cyclomatic_change_counts(
            change_counts=change_counts,
            language=language,
            version=version,
            file_extension=file_extension,
        )
        ax = plt.subplots(figsize=(8, 5))[1]
        plot_data = paired
        if log:
            plot_data = paired[
                (paired["total_cyclomatic"] > 0) & (paired["change_count"] > 0)
            ]
        ax.scatter(
            plot_data["total_cyclomatic"], plot_data["change_count"], alpha=alpha, s=s
        )
        if log:
            ax.set_xscale("log")
            ax.set_yscale("log")
        stats = metric_change_correlation(plot_data, "total_cyclomatic")
        title = (
            f"File change count vs total_cyclomatic, version={version} (n={stats['n']})"
        )
        if language is not None:
            title += f" - {_language_value(language)}"
        title += f"\nSpearman r={stats['spearman_r']:.3f}, Pearson r={stats['pearson_r']:.3f}"
        ax.set_title(title)
        ax.set_xlabel("total_cyclomatic")
        ax.set_ylabel("change_count")
        ax.grid(True, alpha=0.25)
        ax.figure.tight_layout()
        return ax

    def resolve_metric(self, query: str, level: str | None = None) -> str:
        """Resolve a metric key from a key, label, or human-ish search string."""
        metrics = self.metric_names(level)
        query_norm = _normalize(query)
        level_name = normalize_level(level) if level else None

        alias = METRIC_ALIASES.get((level_name, query_norm))
        if alias and alias in set(metrics[correlation.METRIC_KEY]):
            return alias

        exact_key = metrics[
            metrics[correlation.METRIC_KEY].map(_normalize) == query_norm
        ]
        if len(exact_key) == 1:
            return str(exact_key.iloc[0][correlation.METRIC_KEY])

        label_col = "metric_label"
        exact_label = metrics[
            metrics[label_col].fillna("").map(_normalize) == query_norm
        ]
        if len(exact_label) == 1:
            return str(exact_label.iloc[0][correlation.METRIC_KEY])

        query_words = set(query_norm.split())
        matches = metrics[
            metrics.apply(
                lambda row: query_words <= set(_metric_search_text(row).split()), axis=1
            )
        ]
        if len(matches) == 1:
            return str(matches.iloc[0][correlation.METRIC_KEY])
        if matches.empty:
            raise ValueError(f"no metric matched {query!r}")

        options = matches[
            [correlation.METRIC_LEVEL_COL, correlation.METRIC_KEY, "metric_label"]
        ].to_string(index=False)
        raise ValueError(f"metric query {query!r} is ambiguous:\n{options}")

    def _metrics_for_level(self, level: str | None) -> pd.DataFrame:
        level_name = normalize_level(level) if level else None
        if level_name == "version":
            df = self.version_metrics
        else:
            df = self.raw_metrics
        if level_name is None:
            return df.copy()
        return df[df[correlation.METRIC_LEVEL_COL] == level_name].copy()


_DEFAULT_DATA: MetricsData | None = None


def load_data(root: str | Path = ".") -> MetricsData:
    """Load all metrics CSV files for interactive use."""
    root = Path(root)
    raw_paths = sorted(
        path
        for path in root.glob("metrics*.csv")
        if path.stem.removeprefix("metrics").isdigit()
    )
    raw_metrics = correlation.load_raw_metrics(raw_paths)
    version_metrics = correlation.load_metrics_csv(root / "version-metrics.csv")
    return MetricsData(
        raw_metrics=raw_metrics, version_metrics=version_metrics, root=root
    )


def data(root: str | Path = ".", reload: bool = False) -> MetricsData:
    """Return a cached MetricsData object."""
    global _DEFAULT_DATA
    if reload or _DEFAULT_DATA is None or Path(root) != _DEFAULT_DATA.root:
        _DEFAULT_DATA = load_data(root)
    return _DEFAULT_DATA


def sample(*args, **kwargs) -> pd.DataFrame:
    """Convenience shortcut for data().sample(...)."""
    return data().sample(*args, **kwargs)


def sample_urls(*args, **kwargs) -> pd.DataFrame:
    """Convenience shortcut for data().sample_urls(...)."""
    return data().sample_urls(*args, **kwargs)


def sample_median_by_language(*args, **kwargs) -> pd.DataFrame:
    """Convenience shortcut for data().sample_median_by_language(...)."""
    return data().sample_median_by_language(*args, **kwargs)


def sample_median_urls_by_language(*args, **kwargs) -> pd.DataFrame:
    """Convenience shortcut for data().sample_median_urls_by_language(...)."""
    return data().sample_median_urls_by_language(*args, **kwargs)


def plot_model(*args, **kwargs) -> plt.Axes:
    """Convenience shortcut for data().plot_model(...)."""
    return data().plot_model(*args, **kwargs)


def plot_models(*args, **kwargs) -> plt.Figure:
    """Convenience shortcut for data().plot_models(...)."""
    return data().plot_models(*args, **kwargs)


def plot_file_spread_to_loc(*args, **kwargs) -> plt.Figure:
    """Convenience shortcut for data().plot_file_spread_to_loc(...)."""
    return data().plot_file_spread_to_loc(*args, **kwargs)


def plot_go_ocaml_file_spread_to_loc(*args, **kwargs) -> plt.Figure:
    """Convenience shortcut for data().plot_go_ocaml_file_spread_to_loc(...)."""
    return data().plot_go_ocaml_file_spread_to_loc(*args, **kwargs)


def plot_function_spread_to_loc(*args, **kwargs) -> plt.Figure:
    """Convenience shortcut for data().plot_function_spread_to_loc(...)."""
    return data().plot_function_spread_to_loc(*args, **kwargs)


def plot_go_ocaml_function_spread_to_loc(*args, **kwargs) -> plt.Figure:
    """Convenience shortcut for data().plot_go_ocaml_function_spread_to_loc(...)."""
    return data().plot_go_ocaml_function_spread_to_loc(*args, **kwargs)


def file_loc_change_counts(*args, **kwargs) -> pd.DataFrame:
    """Convenience shortcut for data().file_loc_change_counts(...)."""
    return data().file_loc_change_counts(*args, **kwargs)


def file_cyclomatic_change_counts(*args, **kwargs) -> pd.DataFrame:
    """Convenience shortcut for data().file_cyclomatic_change_counts(...)."""
    return data().file_cyclomatic_change_counts(*args, **kwargs)


def file_loc_change_correlation(*args, **kwargs) -> dict[str, float]:
    """Convenience shortcut for data().file_loc_change_correlation(...)."""
    return data().file_loc_change_correlation(*args, **kwargs)


def file_cyclomatic_change_correlation(*args, **kwargs) -> dict[str, float]:
    """Convenience shortcut for data().file_cyclomatic_change_correlation(...)."""
    return data().file_cyclomatic_change_correlation(*args, **kwargs)


def plot_file_loc_change_counts(*args, **kwargs) -> plt.Axes:
    """Convenience shortcut for data().plot_file_loc_change_counts(...)."""
    return data().plot_file_loc_change_counts(*args, **kwargs)


def plot_file_cyclomatic_change_counts(*args, **kwargs) -> plt.Axes:
    """Convenience shortcut for data().plot_file_cyclomatic_change_counts(...)."""
    return data().plot_file_cyclomatic_change_counts(*args, **kwargs)


def metric_names(level: str | None = None) -> pd.DataFrame:
    """Convenience shortcut for data().metric_names(...)."""
    return data().metric_names(level)


def with_github_urls(
    df: pd.DataFrame,
    ref: str | None = None,
    resolve_refs: bool = False,
    strict: bool = False,
    sourcery_db: str | Path = SOURCERY_DB,
    url_col: str = "github_url",
) -> pd.DataFrame:
    """Return df with GitHub URLs derived from metric rows.

    Use resolve_refs=True to call sourcery-db version-by-sample and turn
    codebase_id/sample_number or version_number into exact commit_hash refs.
    Without ref=..., a commit/ref column, or resolve_refs=True, URLs use HEAD.
    """
    missing = [col for col in ["codebase_name", "file_path"] if col not in df.columns]
    if missing:
        raise ValueError(
            "cannot generate GitHub file URLs; dataframe is missing column(s): "
            + ", ".join(missing)
        )

    result = df.copy()
    row_refs = _github_refs(
        result,
        ref,
        resolve_refs=resolve_refs,
        strict=strict,
        sourcery_db=sourcery_db,
    )
    result[url_col] = [
        _github_url_for_row(row, row_ref)
        for (_, row), row_ref in zip(result.iterrows(), row_refs, strict=False)
    ]
    return result


def load_file_change_counts(root: str | Path = ".") -> pd.DataFrame:
    """Load file-change-count CSVs and normalize file_path/change_count columns."""
    root = Path(root)
    paths = sorted(root.glob("file-change-counts*.csv"))
    if not paths:
        raise ValueError(f"no file-change-counts*.csv files found in {root}")

    frames = []
    for path in paths:
        df = pd.read_csv(path)
        required = {"codebase_id", "file", "change_count"}
        missing = required - set(df.columns)
        if missing:
            raise ValueError(
                f"CSV {path} is missing required columns: " + ", ".join(sorted(missing))
            )
        df = df.rename(columns={"file": "file_path"})
        df["change_count"] = pd.to_numeric(df["change_count"], errors="coerce")
        df = df.dropna(subset=["codebase_id", "file_path", "change_count"])
        path_words = set(_normalize(path.stem).split())
        language = None
        if "go" in path_words:
            language = "Golang"
        elif "ocaml" in path_words:
            language = "Ocaml"
        if language is not None:
            df[correlation.LANGUAGE_COL] = language
        frames.append(
            df[
                [
                    col
                    for col in [
                        "codebase_id",
                        "file_path",
                        "change_count",
                        correlation.LANGUAGE_COL,
                    ]
                    if col in df.columns
                ]
            ]
        )

    if not frames:
        return pd.DataFrame(columns=["codebase_id", "file_path", "change_count"])
    return pd.concat(frames, ignore_index=True)


def loc_change_correlation(df: pd.DataFrame) -> dict[str, float]:
    """Calculate LOC/change_count Pearson and Spearman correlations."""
    return metric_change_correlation(df, "lines_of_code")


def metric_change_correlation(df: pd.DataFrame, metric_col: str) -> dict[str, float]:
    """Calculate metric/change_count Pearson and Spearman correlations."""
    required = {metric_col, "change_count"}
    missing = required - set(df.columns)
    if missing:
        raise ValueError(
            "correlation dataframe is missing column(s): " + ", ".join(sorted(missing))
        )

    paired = df[[metric_col, "change_count"]].dropna()
    paired = paired[
        np.isfinite(paired[metric_col]) & np.isfinite(paired["change_count"])
    ]
    if (
        len(paired) < 3
        or paired[metric_col].nunique() < 2
        or paired["change_count"].nunique() < 2
    ):
        return {
            "n": int(len(paired)),
            "pearson_r": np.nan,
            "pearson_p": np.nan,
            "spearman_r": np.nan,
            "spearman_p": np.nan,
        }

    pearson = correlation.stats.pearsonr(paired[metric_col], paired["change_count"])
    spearman = correlation.stats.spearmanr(paired[metric_col], paired["change_count"])
    return {
        "n": int(len(paired)),
        "pearson_r": float(pearson.statistic),
        "pearson_p": float(pearson.pvalue),
        "spearman_r": float(spearman.statistic),
        "spearman_p": float(spearman.pvalue),
    }


def print_urls(df: pd.DataFrame, url_col: str = "github_url") -> None:
    """Print one URL per line from a dataframe."""
    if url_col not in df.columns:
        raise ValueError(f"dataframe is missing URL column {url_col!r}")
    for url in df[url_col].dropna():
        print(url)


def normalize_level(level: str | None) -> str:
    if level is None:
        raise ValueError("level is required")
    level_name = LEVEL_ALIASES.get(_normalize(level))
    if level_name is None:
        raise ValueError(
            f"unknown metric level {level!r}; use version, file, or function"
        )
    return level_name


def _language_value(language: str | Language) -> str:
    return str(getattr(language, "value", language))


def _columns_for_url_generation(columns: list[str] | None) -> list[str] | None:
    if columns is None:
        return None

    required_columns = [
        "codebase_id",
        "codebase_name",
        "sample_number",
        "version_number",
        "file_path",
        "function_start_line",
        "function_end_line",
    ]
    return list(dict.fromkeys([*columns, *required_columns]))


def _github_refs(
    df: pd.DataFrame,
    ref: str | None,
    resolve_refs: bool,
    strict: bool,
    sourcery_db: str | Path,
) -> pd.Series:
    if ref is not None:
        return pd.Series([ref] * len(df), index=df.index)
    for col in GIT_REF_COLUMNS:
        if col in df.columns:
            refs = df[col].where(df[col].notna(), "HEAD").astype(str)
            return refs.mask(refs.map(_is_blank), "HEAD")
    if resolve_refs:
        return df.apply(
            lambda row: _resolve_row_ref(row, strict=strict, sourcery_db=sourcery_db),
            axis=1,
        )
    return pd.Series(["HEAD"] * len(df), index=df.index)


def _resolve_row_ref(row: pd.Series, strict: bool, sourcery_db: str | Path) -> str:
    codebase_id = row.get("codebase_id")
    if _is_blank(codebase_id):
        if strict:
            raise ValueError("cannot resolve GitHub ref; row is missing codebase_id")
        return "HEAD"

    sample_numbers = []
    for col in ["sample_number", "version_number"]:
        sample_number = _int_or_none(row.get(col))
        if sample_number is not None and sample_number not in sample_numbers:
            sample_numbers.append(sample_number)

    for sample_number in sample_numbers:
        commit_hash = version_commit_hash(str(codebase_id), sample_number, sourcery_db)
        if commit_hash:
            return commit_hash

    if strict:
        raise ValueError(
            "cannot resolve GitHub ref for "
            f"codebase_id={codebase_id!r}, sample_numbers={sample_numbers!r}"
        )
    return "HEAD"


def version_commit_hash(
    codebase_id: str,
    sample_number: int,
    sourcery_db: str | Path = SOURCERY_DB,
) -> str | None:
    """Return commit_hash from sourcery-db version-by-sample, cached per version."""
    cache_key = (codebase_id, sample_number)
    if cache_key in _VERSION_REF_CACHE:
        return _VERSION_REF_CACHE[cache_key]

    completed = subprocess.run(
        [str(sourcery_db), "version-by-sample", codebase_id, str(sample_number)],
        check=True,
        capture_output=True,
        text=True,
    )
    output = completed.stdout.strip()
    if not output or output == "null":
        _VERSION_REF_CACHE[cache_key] = None
        return None

    version = json.loads(output)
    commit_hash = version.get("commit_hash")
    if _is_blank(commit_hash):
        commit_hash = None
    _VERSION_REF_CACHE[cache_key] = commit_hash
    return commit_hash


def _github_url_for_row(row: pd.Series, ref: str) -> str | pd.NA:
    repo = _github_repo(row.get("codebase_name"))
    file_path = row.get("file_path")
    if repo is None or _is_blank(file_path):
        return pd.NA

    owner, name = repo
    encoded_path = quote(str(file_path).lstrip("/"))
    url = (
        f"https://github.com/{quote(owner, safe='')}/{quote(name, safe='')}"
        f"/blob/{quote(str(ref), safe='')}/{encoded_path}"
    )
    line_fragment = _line_fragment(
        row.get("function_start_line"), row.get("function_end_line")
    )
    return url + line_fragment


def _github_repo(value: object) -> tuple[str, str] | None:
    if _is_blank(value):
        return None
    repo_name = str(value).split(" (", 1)[0].strip()
    if "__" not in repo_name:
        return None
    owner, name = repo_name.split("__", 1)
    if not owner or not name:
        return None
    return owner, name


def _line_fragment(start_line: object, end_line: object) -> str:
    start = _int_or_none(start_line)
    end = _int_or_none(end_line)
    if start is None:
        return ""
    if end is None or end == start:
        return f"#L{start}"
    return f"#L{start}-L{end}"


def _int_or_none(value: object) -> int | None:
    if _is_blank(value):
        return None
    try:
        number = float(value)
    except (TypeError, ValueError):
        return None
    if not np.isfinite(number):
        return None
    return int(number)


def _is_blank(value: object) -> bool:
    if value is None:
        return True
    try:
        if pd.isna(value):
            return True
    except ValueError:
        return False
    return str(value).strip() == ""


def _filter_value(
    df: pd.DataFrame,
    value: float,
    op: Literal["==", "=", "!=", "<", "<=", ">", ">="],
) -> pd.DataFrame:
    values = df[correlation.VALUE_COL]
    if op in {"==", "="}:
        return df[np.isclose(values, value)]
    if op == "!=":
        return df[~np.isclose(values, value)]
    if op == "<":
        return df[values < value]
    if op == "<=":
        return df[values <= value]
    if op == ">":
        return df[values > value]
    if op == ">=":
        return df[values >= value]
    raise ValueError(f"unknown operator {op!r}")


def _filter_file_extension(
    df: pd.DataFrame,
    file_extension: str | None,
) -> pd.DataFrame:
    if file_extension is None:
        return df
    if "file_path" not in df.columns:
        raise ValueError("cannot filter by file extension; dataframe is missing file_path")

    extension = str(file_extension).strip()
    if not extension:
        return df
    if not extension.startswith("."):
        extension = f".{extension}"

    return df[df["file_path"].fillna("").astype(str).str.endswith(extension)].copy()


def _sample_evenly_by_project(
    df: pd.DataFrame, count: int, random: bool, seed: int | None
) -> pd.DataFrame:
    if "codebase_id" not in df.columns:
        raise ValueError("cannot distribute sample by project; dataframe is missing codebase_id")

    count = min(count, len(df))
    if count <= 0:
        return df.head(0)

    if random:
        df = df.sample(frac=1, random_state=seed)

    sampled = df.copy()
    sampled["_project_rank"] = sampled.groupby("codebase_id", dropna=False).cumcount()
    sampled["_project_order"] = sampled.groupby("codebase_id", dropna=False).ngroup()
    return (
        sampled.sort_values(["_project_rank", "_project_order"])
        .head(count)
        .drop(columns=["_project_rank", "_project_order"])
    )


def _best_model_name(model_results: dict[str, float]) -> str:
    best_name = ""
    best_r2 = np.nan
    for model_name in correlation.MODEL_NAMES:
        r2 = model_results[f"{model_name}_r2"]
        if pd.isna(r2):
            continue
        if pd.isna(best_r2) or r2 > best_r2:
            best_name = model_name
            best_r2 = r2
    if not best_name:
        raise ValueError("no model could be fit")
    return best_name


def _metric_search_text(row: pd.Series) -> str:
    text = f"{row[correlation.METRIC_KEY]} {row.get('metric_label', '')}"
    if "cyclomatic" in _normalize(text):
        text += " complexity"
    return _normalize(text)


def _normalize(value: object) -> str:
    return " ".join(
        "".join(char.lower() if char.isalnum() else " " for char in str(value)).split()
    )
